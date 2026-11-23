//! Otherwise Helper's jobs, declared in the bundle's `Contents/Resources/otherwise-background.json`
//! (`mi-huerta-cli background manifest`), so they run while the app is closed:
//!
//! - `reminders` (daily 07:00, no network): "Hoy toca regar / abonar / cortar el prado"
//!   notifications through the outbox, supplies running low, and a fresh calendar feed.
//!   Reads the garden only.
//! - `forecast` (every 6 h, network, **only with weather on**): Open-Meteo's forecast for the
//!   coarse location. Writes the garden under its lock, so it stays idle while the app has the
//!   garden open (the app refreshes the forecast itself).
//!
//! Both take otherwise-link's background lease: with the data root in OneDrive, only one
//! computer runs them. Nothing set up (no garden, notifications or weather off) is idle.
use huerta::model::TaskKind;
use huerta::store::{self, Store};
use otherwise_link::background::{self as bg, Job, Label, Lease, Manifest, Notification, Report};
use otherwise_link::{Link, Root};
use std::path::Path;

pub const REMINDERS: &str = "reminders";
pub const FORECAST: &str = "forecast";

fn label(key: &str) -> Label {
    let mut l = Label::en(key, &i18n::tr_in("en", key));
    for lang in ["es", "pt", "fr"] {
        l = l.with(lang, &i18n::tr_in(lang, key));
    }
    l
}

pub fn manifest() -> Manifest {
    let mut reminders = Job::every(REMINDERS, label("cli.bg.reminders_label"), "1d", &["background", "run", REMINDERS]); // i18n-ok: arguments
    reminders.every = None;
    reminders.daily = Some("07:00".into());
    reminders.exe = Some("mi-huerta-cli".into());
    reminders.timeout_secs = Some(60);
    reminders.memory_mb = Some(200);
    let mut forecast = Job::every(FORECAST, label("cli.bg.forecast_label"), "6h", &["background", "run", FORECAST]); // i18n-ok: arguments
    forecast.exe = Some("mi-huerta-cli".into());
    forecast.network = true;
    forecast.while_open = false;
    forecast.timeout_secs = Some(60);
    forecast.memory_mb = Some(200);
    Manifest { schema: 1, app: store::APP.into(), jobs: vec![reminders, forecast] }
}

fn today_link() -> Link {
    Link::new(store::APP).segment("today")
}

fn names(v: &[String]) -> String {
    let shown: Vec<&str> = v.iter().take(4).map(String::as_str).collect();
    let more = v.len().saturating_sub(shown.len());
    let list = shown.join(", "); // i18n-ok: list separator
    if more > 0 { i18n::tn!("cli.bg.and_more", more, list = list) } else { list }
}

/// The notification title for what is due now.
pub fn due_title(kind: TaskKind, plants: &[String]) -> String {
    let p = names(plants);
    match kind {
        TaskKind::Water => i18n::t!("cli.bg.due_water", plants = p),
        TaskKind::Fertilize => i18n::t!("cli.bg.due_fertilize", plants = p),
        TaskKind::Prune => i18n::t!("cli.bg.due_prune", plants = p),
        TaskKind::Mow => i18n::t!("cli.bg.due_mow", plants = p),
        TaskKind::Harvest => i18n::t!("cli.bg.due_harvest", plants = p),
    }
}

/// `background manifest | run <job>`: prints one report line; non-zero only on failure.
pub fn run(args: &[String]) -> i32 {
    let (report, code) = match (args.first().map(String::as_str), args.get(1).map(String::as_str)) {
        (Some("manifest"), _) => {
            print!("{}", manifest().to_json());
            return 0;
        }
        (Some("run"), Some(REMINDERS)) => reminders(&crate::garden_dir(args), &Root::system(), huerta::today()),
        (Some("run"), Some(FORECAST)) => forecast(&crate::garden_dir(args), &Root::system()),
        _ => (Report::status(i18n::t!("cli.bg.unknown_job")), 2),
    };
    println!("{}", report.line());
    code
}

fn lease(root: &Root) -> Result<Lease, Report> {
    match bg::take_lease(root, store::APP) {
        Lease::Elsewhere(host) => Err(Report { status: Some(i18n::t!("cli.bg.elsewhere", host = host)), idle: true }),
        l => Ok(l),
    }
}

pub fn reminders(dir: &Path, root: &Root, today: chrono::NaiveDate) -> (Report, i32) {
    if !dir.join(store::FILE).exists() {
        return (Report::idle(), 0);
    }
    let _lease = match lease(root) {
        Ok(l) => l,
        Err(r) => return (r, 0),
    };
    let g = match Store::open_read_only(dir) {
        Ok(s) => s.garden.clone(),
        Err(e) => return (Report::status(i18n::t!("cli.error", error = e)), 1),
    };
    let cat = huerta::Catalog::builtin();
    let _ = huerta::feeds::publish(root, &g, cat, today, &crate::task_title);
    if !g.settings.notifications {
        return (Report::idle(), 0);
    }
    let due = huerta::feeds::due_now(&g, cat, today);
    for (kind, plants) in &due {
        let n = Notification::new(format!("due-{today}-{}", kind.id()), due_title(*kind, plants)).link(&today_link());
        let _ = bg::notify(root, store::APP, &n);
    }
    let low = g.low_supplies();
    if !low.is_empty() {
        let list: Vec<String> = low.iter().map(|s| s.name.clone()).collect();
        let n = Notification::new(format!("low-{today}"), i18n::t!("cli.bg.low_supplies", supplies = names(&list))).link(&Link::new(store::APP).segment("supplies"));
        let _ = bg::notify(root, store::APP, &n);
    }
    let count: usize = due.iter().map(|(_, p)| p.len()).sum();
    let status = if count == 0 { i18n::t!("cli.bg.nothing_due") } else { i18n::tn!("cli.bg.due_status", count) };
    (Report::status(status), 0)
}

pub fn forecast(dir: &Path, root: &Root) -> (Report, i32) {
    let Ok(peek) = Store::open_read_only(dir) else { return (Report::idle(), 0) };
    let w = peek.garden.settings.weather.clone();
    drop(peek);
    let (true, Some(lat), Some(lon)) = (w.enabled, w.lat, w.lon) else { return (Report::idle(), 0) };
    let _lease = match lease(root) {
        Ok(l) => l,
        Err(r) => return (r, 0),
    };
    let mut s = match Store::open(dir) {
        Ok(s) => s,
        Err(e) => return (Report::status(i18n::t!("cli.error", error = e)), 1),
    };
    if !s.writable() {
        // the app (or another computer) has the garden: it is the writer
        return (Report::status(i18n::t!("cli.bg.app_open")), 0);
    }
    match huerta::weather::fetch_forecast(lat, lon) {
        Ok(f) => {
            let rain: f64 = f.days.iter().filter(|d| d.date >= huerta::today()).map(|d| d.rain_mm).sum();
            huerta::weather::apply(&mut s.garden, f, huerta::today());
            if let Err(e) = s.save() {
                return (Report::status(i18n::t!("cli.error", error = e)), 1);
            }
            let _ = huerta::feeds::publish(root, &s.garden, huerta::Catalog::builtin(), huerta::today(), &crate::task_title);
            (Report::status(i18n::t!("cli.bg.forecast_status", mm = format!("{rain:.0}"))), 0)
        }
        Err(e) => (Report::status(i18n::t!("cli.error", error = otherwise_link::text::truncate_chars(&e, 80))), 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use huerta::model::{Plant, Supply};

    #[test]
    fn manifest_is_valid_and_translated() {
        let m = manifest();
        let back = Manifest::parse(&m.to_json(), "mi-huerta").unwrap();
        assert_eq!(back.jobs.iter().map(|j| j.id.as_str()).collect::<Vec<_>>(), ["reminders", "forecast"]);
        assert_eq!(back.jobs[0].daily.as_deref(), Some("07:00"));
        assert!(back.jobs[1].network && !back.jobs[0].network);
        assert!(back.jobs.iter().all(|j| j.exe.as_deref() == Some("mi-huerta-cli")));
        assert_ne!(back.jobs[0].label.get("es"), back.jobs[0].label.get("en"));
    }

    #[test]
    fn reminders_queue_notifications_in_the_outbox() {
        let base = std::env::temp_dir().join(format!("mi-huerta-bg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (dir, root) = (base.join("garden"), Root::at(base.join("root")));
        let today = chrono::NaiveDate::from_ymd_opt(2026, 11, 23).unwrap();
        assert_eq!(reminders(&dir, &root, today).0, Report::idle(), "no garden yet: idle");
        let mut s = Store::open(&dir).unwrap();
        let mut p = Plant { id: "p1".into(), name: "Tomatera".into(), added: Some(today - chrono::Duration::days(5)), ..Default::default() };
        p.overrides.water_days = Some(2);
        s.garden.plants.push(p);
        s.garden.supplies.push(Supply { id: "s1".into(), name: "Compost".into(), quantity: 0.5, low_at: Some(1.0), ..Default::default() });
        s.save().unwrap();
        drop(s);
        let (r, code) = reminders(&dir, &root, today);
        assert_eq!(code, 0);
        assert!(r.status.is_some());
        let out = bg::drain(&root, "mi-huerta", chrono::Utc::now());
        let ids: Vec<&str> = out.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids.len(), 2, "{ids:?}");
        assert!(ids.contains(&"due-2026-11-23-water") && ids.contains(&"low-2026-11-23"));
        assert!(out.iter().any(|n| n.title.contains("Tomatera")));
        assert!(base.join("root/mi-huerta/events.json").exists(), "the calendar feed is refreshed too");
        // forecast stays idle while weather is off: no network
        assert_eq!(forecast(&dir, &root).0, Report::idle());
        let _ = std::fs::remove_dir_all(&base);
    }
}
