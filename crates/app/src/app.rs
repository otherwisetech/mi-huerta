//! The window's state: the open garden, the tab, forms, toasts, About, and every change the
//! user makes (each one saved at once and mirrored to Otherwise Apps' calendar). Tabs are
//! drawn in `views.rs`.
use crate::forms::{self, Forms};
use crate::theme::{SANS, tokens};
use gpui::*;
use huerta::catalog::{Catalog, Climate, Place, Season};
use huerta::mcp::Proposal;
use huerta::model::{Action, Garden, Id, Lawn, Purchase, RainSource, Supply, SupplyKind, TaskKind};
use huerta::ops::Op;
use huerta::store::{ReadOnly, Store};
use otherwise_agent_panel::ui::AgentPanel;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(crate) const CONTEXT: &str = "MiHuerta";
const TOAST_FOR: Duration = Duration::from_secs(4);
/// A forecast older than this is refreshed when the app opens (weather on only).
const FORECAST_MAX_AGE_H: i64 = 6;

actions!(mi_huerta, [ShowAbout, ShowToday, ShowPlants, ShowJournal, ShowSupplies, ShowLawn, ShowSettings, SaveAs, MoveGarden, OpenGarden, Reveal, CloseModal, ToggleAssistant]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("secondary-1", ShowToday, Some(CONTEXT)),
        KeyBinding::new("secondary-2", ShowPlants, Some(CONTEXT)),
        KeyBinding::new("secondary-3", ShowJournal, Some(CONTEXT)),
        KeyBinding::new("secondary-4", ShowSupplies, Some(CONTEXT)),
        KeyBinding::new("secondary-5", ShowLawn, Some(CONTEXT)),
        KeyBinding::new("secondary-,", ShowSettings, Some(CONTEXT)),
        KeyBinding::new("escape", CloseModal, Some(CONTEXT)),
    ]);
}

pub fn reveal_label() -> String {
    if cfg!(target_os = "macos") {
        i18n::t!("app.file.reveal_macos")
    } else if cfg!(windows) {
        i18n::t!("app.file.reveal_windows")
    } else {
        i18n::t!("app.file.reveal_linux")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Today,
    Plants,
    Journal,
    Supplies,
    Lawn,
    Settings,
}

pub struct HuertaApp {
    pub focus: FocusHandle,
    pub store: Option<Store>,
    pub open_error: Option<String>,
    pub tab: Tab,
    pub forms: Forms,
    pub selected: Option<Id>,
    /// New plant form.
    pub new_species: Option<String>,
    pub new_place: Place,
    /// Journal form.
    pub journal_plant: Option<Id>,
    pub journal_photos: Vec<String>,
    pub buy_supply: Option<Id>,
    pub new_supply_kind: SupplyKind,
    pub toast: Option<(String, Instant)>,
    pub about: bool,
    pub logo: Arc<Image>,
    pub panel: Option<WeakEntity<AgentPanel>>,
    /// Proposals shown as cards, by id.
    pub pending: HashMap<String, Proposal>,
    /// Where `mi-huerta-cli mcp --propose` drops proposals (per-user temp folder).
    pub inbox: PathBuf,
    pub weather_busy: bool,
    pub found: Vec<huerta::weather::Found>,
    /// Which tab's settings were filled into the interval fields.
    pub filled_for: Option<Id>,
}

fn make_inbox() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mi-huerta-proposals-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    dir
}

impl HuertaApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let dir = huerta::local::Local::load().garden_dir();
        let (store, open_error) = match Store::open(&dir) {
            Ok(s) => (Some(s), None),
            Err(e) => (None, Some(i18n::t!("app.error.open", path = dir.display(), error = e))),
        };
        let mut this = HuertaApp {
            focus: cx.focus_handle(),
            store,
            open_error,
            tab: Tab::Today,
            forms: Forms::new(window, cx),
            selected: None,
            new_species: None,
            new_place: Place::Matera,
            journal_plant: None,
            journal_photos: vec![],
            buy_supply: None,
            new_supply_kind: SupplyKind::Abono,
            toast: None,
            about: false,
            logo: Arc::new(Image::from_bytes(ImageFormat::Png, crate::ICON_PNG.to_vec())),
            panel: None,
            pending: HashMap::new(),
            inbox: make_inbox(),
            weather_busy: false,
            found: vec![],
            filled_for: None,
        };
        this.publish();
        this.start_ticker(cx);
        this.refresh_weather_if_stale(cx);
        window.focus(&this.focus, cx);
        this
    }

    pub fn garden(&self) -> Option<&Garden> {
        self.store.as_ref().map(|s| &s.garden)
    }

    pub fn catalog(&self) -> &'static Catalog {
        Catalog::builtin()
    }

    pub fn writable(&self) -> bool {
        self.store.as_ref().is_some_and(|s| s.writable())
    }

    fn publish(&self) {
        if let Some(s) = &self.store {
            let _ = mi_huerta_cli::publish_calendar(&s.garden);
        }
    }

    /// Proposals, deep links, the lock's heartbeat and outside changes.
    fn start_ticker(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let mut n: u64 = 0;
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                n += 1;
                let alive = this.update(cx, |this, cx| {
                    this.poll_proposals(cx);
                    for l in crate::take_inbound() {
                        this.open_link(&l, cx);
                    }
                    if n % 60 == 0 {
                        this.heartbeat(cx);
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn heartbeat(&mut self, cx: &mut Context<Self>) {
        let Some(s) = self.store.as_mut() else { return };
        let was = s.writable();
        let lost = !s.heartbeat() && was;
        if !s.writable() && s.changed_on_disk() && s.reload().is_ok() {
            cx.notify();
        }
        if lost {
            self.show_toast(i18n::t!("app.storage.taken_over"), cx);
        }
    }

    pub fn open_link(&mut self, l: &otherwise_link::Link, cx: &mut Context<Self>) {
        if l.app != huerta::store::APP {
            return;
        }
        match (l.at(0), l.at(1)) {
            (Some("plant"), Some(id)) => {
                self.selected = Some(id.to_string());
                self.tab = Tab::Plants;
            }
            (Some("lawn"), _) => self.tab = Tab::Lawn,
            (Some("supplies"), _) => self.tab = Tab::Supplies,
            (Some("journal"), _) => self.tab = Tab::Journal,
            _ => self.tab = Tab::Today,
        }
        cx.activate(true);
        cx.notify();
    }

    pub fn set_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.tab = tab;
        cx.notify();
    }

    pub fn show_toast(&mut self, text: String, cx: &mut Context<Self>) {
        let at = Instant::now();
        self.toast = Some((text, at));
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TOAST_FOR).await;
            this.update(cx, |this, cx| {
                if this.toast.as_ref().is_some_and(|(_, t)| *t == at) {
                    this.toast = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Changes the garden and saves it at once (the calendar feed follows).
    pub fn change(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Garden) -> Result<(), String>) -> bool {
        let Some(s) = self.store.as_mut() else { return false };
        if !s.writable() {
            self.show_toast(i18n::t!("app.storage.read_only_toast"), cx);
            return false;
        }
        let before = s.garden.clone();
        if let Err(e) = f(&mut s.garden) {
            s.garden = before;
            self.show_toast(i18n::t!("app.error.change", error = e), cx);
            return false;
        }
        if let Err(e) = s.save() {
            s.garden = before;
            self.show_toast(i18n::t!("app.error.save", error = e), cx);
            return false;
        }
        self.publish();
        cx.notify();
        true
    }

    pub fn apply_op(&mut self, op: Op, cx: &mut Context<Self>) -> bool {
        let today = huerta::today();
        let cat = self.catalog();
        self.change(cx, |g| op.apply(g, cat, today).map(|_| ()))
    }

    // ── tasks ────────────────────────────────────────────────────────────────────────────────

    pub fn mark(&mut self, kind: TaskKind, target: Id, action: Action, cx: &mut Context<Self>) {
        let today = huerta::today();
        let msg = match &action {
            Action::Done => i18n::t!("app.today.done_toast", task = mi_huerta_cli::task_label(kind)),
            Action::Skipped => i18n::t!("app.today.skipped_toast"),
            Action::Snoozed { .. } => i18n::t!("app.today.snoozed_toast"),
        };
        if self.change(cx, |g| {
            huerta::schedule::record(g, kind, &target, action, today);
            Ok(())
        }) {
            self.show_toast(msg, cx);
        }
    }

    pub fn log_rain(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mm = forms::number(&self.forms.rain_mm, cx);
        if self.change(cx, |g| {
            g.log_rain(huerta::today(), mm, RainSource::Manual);
            Ok(())
        }) {
            forms::clear(&self.forms.rain_mm, window, cx);
            self.show_toast(i18n::t!("app.today.rain_logged"), cx);
        }
    }

    // ── plants ───────────────────────────────────────────────────────────────────────────────

    pub fn add_plant(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut name = forms::text(&self.forms.plant_name, cx);
        if name.is_empty() {
            match self.new_species.as_deref().and_then(|s| self.catalog().get(s)) {
                Some(sp) => name = sp.name(i18n::locale()).to_string(),
                None => {
                    self.show_toast(i18n::t!("app.plants.need_name"), cx);
                    return;
                }
            }
        }
        let planted = forms::date(&self.forms.plant_planted, cx);
        if planted.is_none() && !forms::text(&self.forms.plant_planted, cx).is_empty() {
            self.show_toast(i18n::t!("app.bad_date"), cx);
            return;
        }
        let notes = Some(forms::text(&self.forms.plant_notes, cx)).filter(|n| !n.is_empty());
        let op = Op::AddPlant { name, species: self.new_species.clone(), place: self.new_place, planted, notes };
        if self.apply_op(op, cx) {
            self.selected = self.garden().and_then(|g| g.plants.last()).map(|p| p.id.clone());
            for f in [&self.forms.plant_name, &self.forms.plant_planted, &self.forms.plant_notes] {
                forms::clear(f, window, cx);
            }
            self.show_toast(i18n::t!("app.plants.added"), cx);
        }
    }

    pub fn select_plant(&mut self, id: Id, cx: &mut Context<Self>) {
        self.selected = Some(id);
        self.filled_for = None;
        cx.notify();
    }

    /// Puts the selected plant's own intervals and notes in the edit fields (once per plant).
    pub fn fill_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.filled_for == self.selected {
            return;
        }
        self.filled_for = self.selected.clone();
        let Some(p) = self.selected.as_deref().and_then(|id| self.garden()?.plant(id)).cloned() else { return };
        let s = |d: Option<u32>| d.map(|d| d.to_string()).unwrap_or_default();
        let f = &self.forms;
        let pairs = [(&f.water_days, s(p.overrides.water_days)), (&f.fertilize_days, s(p.overrides.fertilize_days)), (&f.prune_days, s(p.overrides.prune_days)), (&f.harvest_days, s(p.overrides.harvest_days)), (&f.edit_notes, p.notes.clone())];
        for (st, v) in pairs {
            forms::put(st, v, window, cx);
        }
    }

    pub fn save_plant_edits(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone() else { return };
        let f = &self.forms;
        let (w, fe, pr, h, notes) = (forms::days(&f.water_days, cx), forms::days(&f.fertilize_days, cx), forms::days(&f.prune_days, cx), forms::days(&f.harvest_days, cx), forms::text(&f.edit_notes, cx));
        if self.change(cx, |g| {
            let p = g.plants.iter_mut().find(|p| p.id == id).ok_or("no plant")?;
            p.overrides.water_days = w;
            p.overrides.fertilize_days = fe;
            p.overrides.prune_days = pr;
            p.overrides.harvest_days = h;
            p.notes = notes;
            Ok(())
        }) {
            self.show_toast(i18n::t!("app.saved"), cx);
        }
    }

    pub fn set_plant(&mut self, id: Id, cx: &mut Context<Self>, f: impl FnOnce(&mut huerta::model::Plant)) {
        self.change(cx, |g| {
            let p = g.plants.iter_mut().find(|p| p.id == id).ok_or("no plant")?;
            f(p);
            Ok(())
        });
    }

    fn pick_photos(&mut self, window: &mut Window, cx: &mut Context<Self>, done: fn(&mut HuertaApp, Vec<PathBuf>, &mut Context<HuertaApp>)) {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: true, prompt: Some(i18n::t!("app.photos.pick").into()) });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                this.update(cx, |this, cx| done(this, paths, cx)).ok();
            }
        })
        .detach();
    }

    /// Copies photos into the garden folder (the user's files stay where they are).
    pub fn copy_photos(&mut self, paths: &[PathBuf], cx: &mut Context<Self>) -> Vec<String> {
        let Some(s) = self.store.as_ref().filter(|s| s.writable()) else {
            self.show_toast(i18n::t!("app.storage.read_only_toast"), cx);
            return vec![];
        };
        let today = huerta::today();
        let mut out = vec![];
        let mut failed = None;
        for p in paths {
            match s.add_photo(p, today) {
                Ok(rel) => out.push(rel),
                Err(e) => {
                    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    failed = Some(i18n::t!("app.photos.failed", file = name, error = e));
                }
            }
        }
        if let Some(f) = failed {
            self.show_toast(f, cx);
        }
        out
    }

    pub fn add_plant_photos(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pick_photos(window, cx, |this, paths, cx| {
            let Some(id) = this.selected.clone() else { return };
            this.attach_plant_photos(&id, &paths, cx);
        });
    }

    pub fn attach_plant_photos(&mut self, id: &str, paths: &[PathBuf], cx: &mut Context<Self>) -> usize {
        let rels = self.copy_photos(paths, cx);
        let n = rels.len();
        if n > 0 {
            let id = id.to_string();
            self.set_plant(id, cx, |p| p.photos.extend(rels));
            self.show_toast(i18n::tn!("app.photos.added", n), cx);
        }
        n
    }

    pub fn add_journal_photos(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pick_photos(window, cx, |this, paths, cx| {
            let rels = this.copy_photos(&paths, cx);
            this.journal_photos.extend(rels);
            cx.notify();
        });
    }

    // ── journal, supplies, lawn ──────────────────────────────────────────────────────────────

    pub fn add_journal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let f = &self.forms;
        let note = forms::text(&f.journal_note, cx);
        let pests = Some(forms::text(&f.journal_pests, cx)).filter(|s| !s.is_empty());
        let amount = forms::number(&f.journal_amount, cx);
        let unit = Some(forms::text(&f.journal_unit, cx)).filter(|s| !s.is_empty());
        if note.is_empty() && amount.is_none() && pests.is_none() && self.journal_photos.is_empty() {
            self.show_toast(i18n::t!("app.journal.empty"), cx);
            return;
        }
        let photos = std::mem::take(&mut self.journal_photos);
        let plant = self.journal_plant.clone();
        let today = huerta::today();
        let ok = self.change(cx, |g| {
            g.journal.push(huerta::model::JournalEntry { id: huerta::model::new_id("j"), plant, date: Some(today), note, photos, pests: pests.unwrap_or_default(), harvest: amount.map(|amount| huerta::model::HarvestAmount { amount, unit: unit.unwrap_or_default() }), ..Default::default() });
            Ok(())
        });
        if ok {
            for st in [&self.forms.journal_note, &self.forms.journal_pests, &self.forms.journal_amount, &self.forms.journal_unit] {
                forms::clear(st, window, cx);
            }
            self.show_toast(i18n::t!("app.journal.added"), cx);
        }
    }

    pub fn add_supply(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let f = &self.forms;
        let name = forms::text(&f.supply_name, cx);
        if name.is_empty() {
            self.show_toast(i18n::t!("app.supplies.need_name"), cx);
            return;
        }
        let s = Supply { id: huerta::model::new_id("s"), name, kind: self.new_supply_kind, unit: forms::text(&f.supply_unit, cx), quantity: forms::number(&f.supply_qty, cx).unwrap_or(0.0), low_at: forms::number(&f.supply_low, cx), ..Default::default() };
        if self.change(cx, |g| {
            g.supplies.push(s);
            Ok(())
        }) {
            for st in [&self.forms.supply_name, &self.forms.supply_qty, &self.forms.supply_unit, &self.forms.supply_low] {
                forms::clear(st, window, cx);
            }
        }
    }

    pub fn adjust_supply(&mut self, id: Id, delta: f64, cx: &mut Context<Self>) {
        self.change(cx, |g| {
            let s = g.supplies.iter_mut().find(|s| s.id == id).ok_or("no supply")?;
            s.quantity = (s.quantity + delta).max(0.0);
            Ok(())
        });
    }

    pub fn add_purchase(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(supply) = self.buy_supply.clone() else {
            self.show_toast(i18n::t!("app.supplies.pick_first"), cx);
            return;
        };
        let f = &self.forms;
        let (qty, cost, shop) = (forms::number(&f.buy_qty, cx).unwrap_or(0.0), forms::number(&f.buy_cost, cx).unwrap_or(0.0), forms::text(&f.buy_shop, cx));
        let p = Purchase { id: huerta::model::new_id("b"), supply, date: Some(huerta::today()), quantity: qty, cost, currency: "COP".into(), shop, ..Default::default() };
        if self.change(cx, |g| {
            g.add_purchase(p);
            Ok(())
        }) {
            for st in [&self.forms.buy_qty, &self.forms.buy_cost, &self.forms.buy_shop] {
                forms::clear(st, window, cx);
            }
            self.show_toast(i18n::t!("app.supplies.bought"), cx);
        }
    }

    pub fn add_lawn(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let f = &self.forms;
        let name = forms::text(&f.lawn_name, cx);
        let name = if name.is_empty() { i18n::t!("app.lawn.default_name") } else { name };
        let l = Lawn { id: huerta::model::new_id("l"), name, mow_days: forms::days(&f.lawn_days, cx), height_cm: forms::number(&f.lawn_height, cx), ..Default::default() };
        if self.change(cx, |g| {
            g.lawns.push(l);
            Ok(())
        }) {
            for st in [&self.forms.lawn_name, &self.forms.lawn_days, &self.forms.lawn_height] {
                forms::clear(st, window, cx);
            }
        }
    }

    // ── settings and weather ─────────────────────────────────────────────────────────────────

    pub fn set_climate(&mut self, c: Option<Climate>, cx: &mut Context<Self>) {
        self.change(cx, |g| {
            g.settings.climate = c;
            Ok(())
        });
    }

    pub fn set_season(&mut self, s: Season, cx: &mut Context<Self>) {
        self.change(cx, |g| {
            g.settings.season = s;
            Ok(())
        });
    }

    pub fn set_weather(&mut self, on: bool, cx: &mut Context<Self>) {
        if self.change(cx, |g| {
            g.settings.weather.enabled = on;
            if !on {
                g.forecast = None;
            }
            Ok(())
        }) && on
        {
            self.refresh_weather(cx);
        }
    }

    pub fn search_city(&mut self, cx: &mut Context<Self>) {
        let q = forms::text(&self.forms.city, cx);
        if q.is_empty() {
            return;
        }
        let lang = i18n::locale().to_string();
        self.weather_busy = true;
        cx.notify();
        let task = cx.background_executor().spawn(async move { huerta::weather::search(&q, &lang) });
        cx.spawn(async move |this, cx| {
            let res = task.await;
            this.update(cx, |this, cx| {
                this.weather_busy = false;
                match res {
                    Ok(v) if v.is_empty() => this.show_toast(i18n::t!("app.settings.city_none"), cx),
                    Ok(v) => this.found = v,
                    Err(e) => this.show_toast(i18n::t!("app.weather.failed", error = e), cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn choose_city(&mut self, f: huerta::weather::Found, cx: &mut Context<Self>) {
        self.found.clear();
        if self.change(cx, |g| {
            g.settings.weather.place = format!("{}, {}", f.name, f.region); // i18n-ok: place label
            g.settings.weather.lat = Some(huerta::weather::round_coord(f.lat));
            g.settings.weather.lon = Some(huerta::weather::round_coord(f.lon));
            Ok(())
        }) {
            self.refresh_weather(cx);
        }
    }

    pub fn save_skip_mm(&mut self, cx: &mut Context<Self>) {
        let Some(mm) = forms::number(&self.forms.skip_mm, cx).filter(|m| (0.5..=100.0).contains(m)) else {
            self.show_toast(i18n::t!("app.settings.skip_bad"), cx);
            return;
        };
        if self.change(cx, |g| {
            g.settings.weather.skip_mm = mm;
            Ok(())
        }) {
            self.show_toast(i18n::t!("app.saved"), cx);
        }
    }

    fn refresh_weather_if_stale(&mut self, cx: &mut Context<Self>) {
        let Some(g) = self.garden() else { return };
        if !g.settings.weather.enabled {
            return;
        }
        let fresh = g.forecast.as_ref().and_then(|f| chrono::DateTime::parse_from_rfc3339(&f.fetched_at).ok()).is_some_and(|at| chrono::Utc::now().signed_duration_since(at).num_hours() < FORECAST_MAX_AGE_H);
        if !fresh {
            self.refresh_weather(cx);
        }
    }

    /// Network, only with weather on: the coarse location goes to Open-Meteo.
    pub fn refresh_weather(&mut self, cx: &mut Context<Self>) {
        let Some(w) = self.garden().map(|g| g.settings.weather.clone()) else { return };
        let (true, Some(lat), Some(lon)) = (w.enabled, w.lat, w.lon) else { return };
        self.weather_busy = true;
        cx.notify();
        let task = cx.background_executor().spawn(async move { huerta::weather::fetch_forecast(lat, lon) });
        cx.spawn(async move |this, cx| {
            let res = task.await;
            this.update(cx, |this, cx| {
                this.weather_busy = false;
                match res {
                    Ok(f) => {
                        this.change(cx, |g| {
                            huerta::weather::apply(g, f, huerta::today());
                            Ok(())
                        });
                    }
                    Err(e) => this.show_toast(i18n::t!("app.weather.failed", error = e), cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn set_language(&mut self, code: &'static str, cx: &mut Context<Self>) {
        if i18n::set_locale(code) {
            huerta::local::Local::update(|l| l.language = Some(code.to_string()));
            gpui_component::set_locale(code);
            crate::agent::set_panel_locale(code);
            crate::set_app_menus(cx);
            cx.refresh_windows();
        }
    }

    // ── the garden folder ────────────────────────────────────────────────────────────────────

    fn pick_folder(&mut self, prompt: String, window: &mut Window, cx: &mut Context<Self>, done: fn(&mut HuertaApp, PathBuf, &mut Context<HuertaApp>)) {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some(prompt.into()) });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(p) = paths.into_iter().next()
            {
                this.update(cx, |this, cx| done(this, p, cx)).ok();
            }
        })
        .detach();
    }

    /// Save As / Move pick a parent folder; the garden goes into `<it>/Mi Huerta`.
    fn target_in(parent: PathBuf) -> PathBuf {
        if parent.join(huerta::store::FILE).exists() || std::fs::read_dir(&parent).map(|mut d| d.next().is_none()).unwrap_or(false) { parent } else { parent.join(crate::APP_NAME) }
    }

    fn remember_dir(&self) {
        if let Some(s) = &self.store {
            let dir = s.dir.clone();
            let custom = dir != huerta::store::default_dir();
            huerta::local::Local::update(|l| l.garden_dir = custom.then_some(dir));
        }
    }

    pub fn save_as(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pick_folder(i18n::t!("app.file.save_as_prompt"), window, cx, |this, p, cx| {
            let dest = Self::target_in(p);
            let res = this.store.as_mut().map(|s| s.save_as(&dest));
            match res {
                Some(Ok(())) => {
                    this.remember_dir();
                    this.publish();
                    this.show_toast(i18n::t!("app.file.saved_as", path = dest.display()), cx);
                }
                Some(Err(e)) => this.show_toast(i18n::t!("app.error.save", error = e), cx),
                None => {}
            }
        });
    }

    pub fn move_garden(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pick_folder(i18n::t!("app.file.move_prompt"), window, cx, |this, p, cx| {
            let dest = Self::target_in(p);
            let res = this.store.as_mut().map(|s| s.move_to(&dest));
            match res {
                Some(Ok(())) => {
                    this.remember_dir();
                    this.show_toast(i18n::t!("app.file.moved", path = dest.display()), cx);
                }
                Some(Err(e)) => this.show_toast(i18n::t!("app.error.move", error = e), cx),
                None => {}
            }
        });
    }

    pub fn open_garden(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pick_folder(i18n::t!("app.file.open_prompt"), window, cx, |this, p, cx| {
            this.store = None;
            match Store::open(&p) {
                Ok(s) => {
                    this.store = Some(s);
                    this.open_error = None;
                    this.selected = None;
                    this.remember_dir();
                    this.publish();
                }
                Err(e) => this.open_error = Some(i18n::t!("app.error.open", path = p.display(), error = e)),
            }
            cx.notify();
        });
    }

    pub fn reveal(&mut self, cx: &mut Context<Self>) {
        if let Some(s) = &self.store {
            cx.reveal_path(&s.file());
        }
    }

    pub fn take_over(&mut self, cx: &mut Context<Self>) {
        if let Some(s) = self.store.as_mut() {
            match s.take_over() {
                Ok(()) => self.show_toast(i18n::t!("app.storage.taken_here"), cx),
                Err(e) => self.show_toast(i18n::t!("app.error.save", error = e), cx),
            }
        }
        cx.notify();
    }

    pub fn read_only_text(&self) -> Option<String> {
        Some(match self.store.as_ref()?.read_only.as_ref()? {
            ReadOnly::Elsewhere(host) => i18n::t!("app.storage.elsewhere", host = host),
            ReadOnly::Here => i18n::t!("app.storage.here"),
            ReadOnly::Newer(_) => i18n::t!("app.storage.newer"),
            ReadOnly::TakenOver => i18n::t!("app.storage.taken_over"),
            ReadOnly::Asked => i18n::t!("app.storage.read_only_toast"),
        })
    }
}

impl Render for HuertaApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens(cx);
        if cx.try_global::<otherwise_agent_panel::ui::PanelStyle>().is_none_or(|s| s.dark != t.dark) {
            cx.set_global(crate::agent::style(&t));
        }
        if self.tab == Tab::Plants {
            self.fill_selected(window, cx);
        }
        let body = self.render_body(&t, cx);
        div()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &ShowAbout, _, cx| {
                this.about = true;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &CloseModal, _, cx| {
                this.about = false;
                this.found.clear();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ShowToday, _, cx| this.set_tab(Tab::Today, cx)))
            .on_action(cx.listener(|this, _: &ShowPlants, _, cx| this.set_tab(Tab::Plants, cx)))
            .on_action(cx.listener(|this, _: &ShowJournal, _, cx| this.set_tab(Tab::Journal, cx)))
            .on_action(cx.listener(|this, _: &ShowSupplies, _, cx| this.set_tab(Tab::Supplies, cx)))
            .on_action(cx.listener(|this, _: &ShowLawn, _, cx| this.set_tab(Tab::Lawn, cx)))
            .on_action(cx.listener(|this, _: &ShowSettings, _, cx| this.set_tab(Tab::Settings, cx)))
            .on_action(cx.listener(|this, _: &SaveAs, window, cx| this.save_as(window, cx)))
            .on_action(cx.listener(|this, _: &MoveGarden, window, cx| this.move_garden(window, cx)))
            .on_action(cx.listener(|this, _: &OpenGarden, window, cx| this.open_garden(window, cx)))
            .on_action(cx.listener(|this, _: &Reveal, _, cx| this.reveal(cx)))
            .on_action(cx.listener(|this, _: &ToggleAssistant, window, cx| this.toggle_panel(window, cx)))
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(t.canvas)
            .text_color(t.ink)
            .font_family(SANS)
            .child(div().flex().flex_1().min_h_0().child(self.render_sidebar(&t, cx)).child(div().flex_1().min_w_0().child(body)))
            .child(self.render_footer(&t, cx))
            .children(self.render_toast(&t))
            .children(self.about.then(|| self.render_about(&t, cx)))
    }
}
