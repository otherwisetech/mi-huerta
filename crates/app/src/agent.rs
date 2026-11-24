//! The AI panel (otherwise-agent-panel), docked next to every tab and toggled with ⌘J: the
//! user's own Claude Code, Codex or opencode with `mi-huerta-cli mcp` attached for that session
//! only. The garden server is read-only. Outside "Solo lectura" it also offers `propose_*`
//! tools: each proposal lands in this app's inbox (a per-user temp folder) and shows as an
//! Aplicar / Rechazar card; only an accepted card changes the garden. "Automático" proposes
//! too: a garden change always goes through a card.
use crate::app::{HuertaApp, Tab};
use crate::theme::Tokens;
use gpui::*;
use huerta::mcp::Proposal;
use huerta::ops::{Mark, Op};
use otherwise_agent_panel::ui::{AgentHost, AgentPanel, DropAction, DropOutcome, PanelStyle};
use otherwise_agent_panel::{McpServer, PendingChange, PermissionMode};
use std::path::{Path, PathBuf};

const INSTRUCTIONS: &str = "This is Mi Huerta, a home-garden app (tomatoes, herbs, fruit trees, potted plants, the lawn), mostly for Colombia: places are matera (pot), huerta (vegetable bed), jardin (garden), prado (lawn); climates are the Colombian thermal floors calido / templado / frio; seasons are seca / lluviosa. The MCP server \"mi-huerta\" reads the garden: plants, care tasks, journal, supplies, weather. The catalogue's care numbers cite sources (FAO, UF/IFAS, ICA, Agrosavia…): quote the source when you use a number, and when the catalogue has only a hint, say so instead of inventing intervals or doses. For pests and diseases, describe what you see in the user's photo carefully, suggest gentle home-garden measures first, and recommend a local agronomist (ICA, UMATA) for anything serious. To change the garden use the propose_* tools: they become cards the user accepts; never claim a change was made. Answer in the user's language, warmly and plainly."; // i18n-ok: agent instructions (English)

/// The command line inside the bundle (`Contents/MacOS/mi-huerta-cli`), else this binary (which
/// answers the same subcommands).
pub fn bundled_cli() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    let cli = exe.with_file_name(if cfg!(windows) { "mi-huerta-cli.exe" } else { "mi-huerta-cli" }); // i18n-ok: file name
    if cli.is_file() { cli } else { exe }
}

/// `mi-huerta-cli mcp` for a session in `mode`: read-only, or proposing into `inbox` when the
/// garden is writable here.
pub fn mcp_server(cli: &Path, garden: &Path, inbox: &Path, mode: PermissionMode, writable: bool) -> McpServer {
    let (garden, inbox) = (garden.display().to_string(), inbox.display().to_string());
    let mut args = vec!["mcp", "--garden", garden.as_str()]; // i18n-ok: command line
    if mode != PermissionMode::ReadOnly && writable {
        args.extend(["--propose", "--inbox", inbox.as_str()]); // i18n-ok: flags
    }
    McpServer::new(huerta::store::APP, cli, &args)
}

/// The panel's own strings (es / en / pt / fr) follow the app, with a privacy note that says
/// what of the garden is shared.
pub fn set_panel_locale(code: &str) {
    otherwise_agent_panel::i18n::set_locale(code);
    otherwise_agent_panel::i18n::override_label("panel.privacy.cli", i18n::t!("app.agent.privacy"));
}

pub struct Host {
    pub app: WeakEntity<HuertaApp>,
}

fn is_photo(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| huerta::store::PHOTO_EXTS.contains(&e.to_ascii_lowercase().as_str()))
}

impl AgentHost for Host {
    fn app_id(&self) -> String {
        huerta::store::APP.into()
    }

    fn app_name(&self) -> String {
        crate::APP_NAME.into()
    }

    fn mcp_servers(&self, mode: PermissionMode, cx: &App) -> Vec<McpServer> {
        let Some(app) = self.app.upgrade() else { return Vec::new() };
        let a = app.read(cx);
        let Some(s) = a.store.as_ref() else { return Vec::new() };
        vec![mcp_server(&bundled_cli(), &s.dir, &a.inbox, mode, s.writable())]
    }

    fn context(&self, cx: &App) -> Option<String> {
        Some(self.app.upgrade()?.read(cx).agent_context())
    }

    fn instructions(&self) -> Option<String> {
        Some(INSTRUCTIONS.into())
    }

    fn suggestions(&self) -> Vec<String> {
        vec![i18n::t!("app.agent.suggest_tomato"), i18n::t!("app.agent.suggest_week"), i18n::t!("app.agent.suggest_lemon")]
    }

    fn drop_actions(&self, files: &[PathBuf], cx: &App) -> Vec<DropAction> {
        if !files.iter().all(|f| is_photo(f)) {
            return Vec::new();
        }
        let Some(app) = self.app.upgrade() else { return Vec::new() };
        let a = app.read(cx);
        let mut v = vec![DropAction::new("ask", i18n::t!("app.agent.drop_ask"))]; // i18n-ok: action id
        if let Some(p) = a.selected.as_deref().and_then(|id| a.garden()?.plant(id)).filter(|_| a.writable()) {
            v.push(DropAction::new("photo", i18n::t!("app.agent.drop_photo", plant = p.name))); // i18n-ok: action id
        }
        v
    }

    fn run_drop_action(&self, action: &str, files: &[PathBuf], _window: &mut Window, cx: &mut App) -> DropOutcome {
        let plant = self.app.upgrade().and_then(|a| {
            let a = a.read(cx);
            a.selected.as_deref().and_then(|id| a.garden()?.plant(id)).map(|p| (p.id.clone(), p.name.clone()))
        });
        match action {
            "photo" => {
                let Some((id, _)) = plant else { return DropOutcome::Attach };
                match self.app.update(cx, |a, cx| a.attach_plant_photos(&id, files, cx)) {
                    Ok(n) if n > 0 => DropOutcome::Done(Some(i18n::tn!("app.photos.added", n))),
                    _ => DropOutcome::Failed(i18n::t!("app.photos.none")),
                }
            }
            _ => {
                let name = plant.map(|(_, n)| n).unwrap_or_else(|| i18n::t!("app.agent.my_plant"));
                DropOutcome::Ask { prompt: i18n::t!("app.agent.ask_photo", plant = name), send: false }
            }
        }
    }

    fn resolve_change(&self, id: &str, approve: bool, _window: &mut Window, cx: &mut App) -> Result<(), String> {
        self.app.update(cx, |a, cx| a.resolve_proposal(id, approve, cx)).map_err(|e| e.to_string())?
    }
}

/// The panel in Mi Huerta's colours.
pub fn style(t: &Tokens) -> PanelStyle {
    PanelStyle {
        background: t.canvas,
        surface: t.surface,
        raised: t.raised,
        border: t.border,
        ink: t.ink,
        muted: t.ink_muted,
        accent: t.accent,
        accent_soft: t.accent_soft,
        danger: t.danger,
        danger_soft: t.danger_soft,
        ok: t.ok,
        note: t.warn,
        note_soft: t.warn_soft,
        terminal_bg: if t.dark { rgb(0x161514).into() } else { rgb(0xFFFFFF).into() },
        terminal_fg: t.ink,
        mono_font: otherwise_ui::tokens::MONO.into(),
        dark: t.dark,
    }
}

impl HuertaApp {
    fn agent_panel(&self) -> Option<Entity<AgentPanel>> {
        self.panel.as_ref().and_then(|p| p.upgrade())
    }

    pub fn toggle_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = self.agent_panel() {
            p.update(cx, |p, cx| p.toggle(window, cx));
        }
    }

    /// A starter question from a button ("¿Qué toca esta semana?").
    pub fn ask(&mut self, question: String, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(p) = self.agent_panel() {
            p.update(cx, |p, cx| p.ask(question, window, cx));
        }
    }

    /// "Lo que estoy viendo" (English, for the agent).
    pub fn agent_context(&self) -> String {
        let tab = match self.tab {
            Tab::Today => "Today (care tasks: overdue, today, this week)",
            Tab::Plants => "Plants",
            Tab::Journal => "Journal",
            Tab::Supplies => "Supplies",
            Tab::Lawn => "Lawn",
            Tab::Settings => "Settings",
        }; // i18n-ok: context for the agent (English)
        let mut v = serde_json::json!({ "tab": tab, "today": huerta::today() });
        if let Some(g) = self.garden() {
            v["climate"] = serde_json::json!(g.settings.climate);
            v["season"] = serde_json::json!(g.settings.season);
            if let Some(p) = self.selected.as_deref().and_then(|id| g.plant(id)) {
                v["selected_plant"] = serde_json::json!({"id": p.id, "name": p.name, "species": p.species, "place": p.place});
            }
        }
        v.to_string()
    }

    /// New proposals from the MCP server become cards.
    pub fn poll_proposals(&mut self, cx: &mut Context<Self>) {
        let new = huerta::mcp::take_proposals(&self.inbox);
        if new.is_empty() {
            return;
        }
        let Some(panel) = self.agent_panel() else { return };
        for p in new {
            let (title, detail) = self.proposal_text(&p);
            let change = PendingChange::new(p.id.clone(), title, detail);
            self.pending.insert(p.id.clone(), p);
            panel.update(cx, |panel, cx| panel.report_pending_change(change, cx));
        }
    }

    fn target(&self, id: &str) -> String {
        self.garden().and_then(|g| g.target_name(id)).unwrap_or(id).to_string()
    }

    pub fn proposal_text(&self, p: &Proposal) -> (String, String) {
        let title = match &p.op {
            Op::AddPlant { name, .. } => i18n::t!("app.card.add_plant", name = name),
            Op::MarkTask { task, target, mark, days } => {
                let what = format!("{} · {}", mi_huerta_cli::task_label(*task), self.target(target)); // i18n-ok: separator
                match mark {
                    Mark::Done => i18n::t!("app.card.done", task = what),
                    Mark::Skip => i18n::t!("app.card.skip", task = what),
                    Mark::Snooze => i18n::t!("app.card.snooze", task = what, days = i18n::tn!("common.days", days.unwrap_or(1))),
                }
            }
            Op::AddJournal { plant, .. } => i18n::t!("app.card.journal", plant = plant.as_deref().map(|p| self.target(p)).unwrap_or_else(|| i18n::t!("app.journal.whole_garden"))),
            Op::AddSupply { name, .. } => i18n::t!("app.card.supply", name = name),
            Op::LogRain { mm, .. } => match mm {
                Some(mm) => i18n::t!("app.card.rain_mm", mm = mm),
                None => i18n::t!("app.card.rain"),
            },
            Op::SetInterval { target, task, days } => i18n::t!("app.card.interval", task = mi_huerta_cli::task_label(*task), plant = self.target(target), days = i18n::tn!("common.days", *days)),
        };
        let mut detail = Vec::new();
        if let Op::AddJournal { note, .. } = &p.op
            && !note.is_empty()
        {
            detail.push(note.clone());
        }
        if !p.why.is_empty() {
            detail.push(p.why.clone());
        }
        (title, detail.join("\n"))
    }

    /// Aplicar / Rechazar on a card.
    pub fn resolve_proposal(&mut self, id: &str, approve: bool, cx: &mut Context<Self>) -> Result<(), String> {
        let Some(p) = self.pending.remove(id) else {
            return if approve { Err(i18n::t!("app.card.gone")) } else { Ok(()) };
        };
        if !approve {
            return Ok(());
        }
        if !self.writable() {
            return Err(i18n::t!("app.storage.read_only_toast"));
        }
        let today = huerta::today();
        let cat = self.catalog();
        let mut err = None;
        self.change(cx, |g| {
            p.op.apply(g, cat, today).map(|_| ()).inspect_err(|e| err = Some(e.clone()))
        });
        match err {
            Some(e) => Err(e),
            None => {
                self.show_toast(i18n::t!("app.card.applied"), cx);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PermissionMode, mcp_server};
    use std::path::Path;

    #[test]
    fn read_only_unless_the_mode_and_the_garden_allow_proposals() {
        let cli = Path::new("/Applications/Mi Huerta.app/Contents/MacOS/mi-huerta-cli");
        let (g, inbox) = (Path::new("/g"), Path::new("/tmp/in"));
        let args = |m, w| mcp_server(cli, g, inbox, m, w).args;
        assert_eq!(args(PermissionMode::ReadOnly, true), ["mcp", "--garden", "/g"]);
        assert_eq!(args(PermissionMode::Ask, true), ["mcp", "--garden", "/g", "--propose", "--inbox", "/tmp/in"]);
        assert_eq!(args(PermissionMode::Auto, true), ["mcp", "--garden", "/g", "--propose", "--inbox", "/tmp/in"], "automatic still proposes");
        assert_eq!(args(PermissionMode::Ask, false), ["mcp", "--garden", "/g"], "a read-only garden only reads");
        assert_eq!(mcp_server(cli, g, inbox, PermissionMode::Ask, true).name, "mi-huerta");
    }
}
