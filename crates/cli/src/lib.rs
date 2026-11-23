//! `mi-huerta-cli`: the headless side of Mi Huerta (no GPUI), also answered by the app binary.
//!
//! - `mcp [--garden DIR] [--propose --inbox DIR]`: the garden's MCP server (read-only; with an
//!   inbox, `propose_*` tools whose proposals the app shows as cards)
//! - `background manifest | run reminders | run forecast`: Otherwise Helper's jobs
//! - `calendar`: rewrites `<data root>/mi-huerta/events.json`
//! - `tasks [--json]`: what is due this week
pub mod background;

use huerta::local::Local;
use huerta::model::TaskKind;
use std::path::PathBuf;

/// The translated verb of a task ("Regar").
pub fn task_label(k: TaskKind) -> String {
    match k {
        TaskKind::Water => i18n::t!("common.task.water"),
        TaskKind::Fertilize => i18n::t!("common.task.fertilize"),
        TaskKind::Prune => i18n::t!("common.task.prune"),
        TaskKind::Mow => i18n::t!("common.task.mow"),
        TaskKind::Harvest => i18n::t!("common.task.harvest"),
    }
}

/// "Regar · Tomatera", for the calendar.
pub fn task_title(t: &huerta::schedule::Task) -> String {
    format!("{} · {}", task_label(t.kind), t.name) // i18n-ok: separator
}

fn flag(args: &[String], name: &str) -> Option<PathBuf> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(PathBuf::from)
}

/// The garden folder: `--garden DIR`, else the one in the local settings, else the default.
pub fn garden_dir(args: &[String]) -> PathBuf {
    flag(args, "--garden").unwrap_or_else(|| Local::load().garden_dir())
}

/// Publishes the calendar feed from `g` (when the user keeps it on).
pub fn publish_calendar(g: &huerta::Garden) -> std::io::Result<usize> {
    huerta::feeds::publish(&otherwise_link::Root::system(), g, huerta::Catalog::builtin(), huerta::today(), &task_title)
}

fn set_language() {
    let lang = std::env::var(otherwise_link::background::ENV_LANG).ok().filter(|l| !l.is_empty()).or_else(|| Local::load().language);
    if let Some(l) = lang.as_deref().and_then(i18n::match_system) {
        i18n::set_locale(l);
    }
}

/// Runs a command-line subcommand; `None` when `args` isn't one (the app then opens its window).
pub fn run(args: &[String]) -> Option<i32> {
    let cmd = args.first()?.as_str();
    if !matches!(cmd, "mcp" | "background" | "calendar" | "tasks") {
        return None;
    }
    set_language();
    Some(match cmd {
        "mcp" => {
            let inbox = if args.iter().any(|a| a == "--propose") { flag(args, "--inbox") } else { None };
            let srv = huerta::mcp::Server { dir: garden_dir(args), catalog: huerta::Catalog::builtin(), inbox, today: huerta::today };
            match srv.serve(std::io::stdin().lock(), std::io::stdout().lock()) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("mcp: {e}"); // i18n-ok: developer diagnostic
                    1
                }
            }
        }
        "background" => background::run(&args[1..]),
        "calendar" => match huerta::store::Store::open_read_only(&garden_dir(args)).and_then(|s| publish_calendar(&s.garden)) {
            Ok(n) => {
                println!("{}", i18n::tn!("cli.calendar.published", n));
                0
            }
            Err(e) => {
                eprintln!("{}", i18n::t!("cli.error", error = e));
                1
            }
        },
        _ => tasks(args),
    })
}

fn tasks(args: &[String]) -> i32 {
    let g = match huerta::store::Store::open_read_only(&garden_dir(args)) {
        Ok(s) => s.garden.clone(),
        Err(e) => {
            eprintln!("{}", i18n::t!("cli.error", error = e));
            return 1;
        }
    };
    let today = huerta::today();
    let a = huerta::schedule::agenda(&g, huerta::Catalog::builtin(), today);
    let week = a.this_week(today);
    if args.iter().any(|a| a == "--json") {
        println!("{}", serde_json::json!({"ok": true, "today": today, "tasks": week}));
        return 0;
    }
    if week.is_empty() {
        println!("{}", i18n::t!("cli.tasks.none"));
    }
    for t in week {
        println!("{}  {}", t.due, task_title(t)); // i18n-ok: table row
    }
    0
}

pub fn usage() -> String {
    i18n::t!("cli.usage")
}
