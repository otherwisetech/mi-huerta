//! Mi Huerta's part of the shared Otherwise layer:
//!
//! - **Calendar** (`<data root>/mi-huerta/events.json`, otherwise-link `events`): each care
//!   task's next day, repeating every interval (`FREQ=DAILY;INTERVAL=n`), harvest windows as
//!   ranges. Garden chores aren't private: titles say what and which plant.
//! - **Daily reminders** for Otherwise Helper's notifications: what is due today or overdue,
//!   grouped by kind ("Hoy toca regar: …").
//!
//! Texts are translated by the caller (core has no UI strings).
use crate::catalog::Catalog;
use crate::model::{Garden, TaskKind};
use crate::schedule::{self, Bucket, Task};
use chrono::{Duration, NaiveDate};
use otherwise_link::events::{Event, Kind, Status};
use otherwise_link::{Link, Root};

pub const APP: &str = crate::store::APP;
/// How far ahead one-off events (harvests) are published.
const AHEAD_DAYS: i64 = 400;

/// `otherwise://mi-huerta/plant/<id>` (or `lawn/<id>`).
pub fn target_link(g: &Garden, id: &str) -> Link {
    Link::new(APP).segment(if g.lawn(id).is_some() { "lawn" } else { "plant" }).segment(id)
}

/// The calendar events; `title(task)` gives the translated title ("Regar · Tomatera").
pub fn events(g: &Garden, cat: &Catalog, today: NaiveDate, title: &dyn Fn(&Task) -> String) -> Vec<Event> {
    let a = schedule::agenda(g, cat, today);
    let mut out = Vec::new();
    for t in a.tasks.iter().filter(|t| t.due <= today + Duration::days(AHEAD_DAYS)) {
        let kind = if t.kind == TaskKind::Harvest { Kind::Task } else { Kind::Maintenance };
        let mut e = Event::day(t.key(), kind, title(t), t.due).link(&target_link(g, &t.target)).category(t.kind.id()).status(if t.bucket(today) == Bucket::Overdue { Status::Tentative } else { Status::Confirmed });
        if let Some(until) = t.until {
            e = e.end((until + Duration::days(1)).to_string());
        }
        if let Some(n) = t.interval.filter(|_| t.kind != TaskKind::Harvest) {
            e = e.rrule(format!("FREQ=DAILY;INTERVAL={n}"));
        }
        out.push(e);
    }
    out
}

pub fn publish(root: &Root, g: &Garden, cat: &Catalog, today: NaiveDate, title: &dyn Fn(&Task) -> String) -> std::io::Result<usize> {
    if !g.settings.calendar {
        otherwise_link::events::clear(root, APP)?;
        return Ok(0);
    }
    let v = events(g, cat, today, title);
    otherwise_link::events::publish(root, APP, &v, None)?;
    Ok(v.len())
}

/// Due today or overdue, by kind, with the plant / lawn names (each kind once).
pub fn due_now(g: &Garden, cat: &Catalog, today: NaiveDate) -> Vec<(TaskKind, Vec<String>)> {
    let a = schedule::agenda(g, cat, today);
    let mut out: Vec<(TaskKind, Vec<String>)> = Vec::new();
    for t in a.tasks.iter().filter(|t| t.due <= today) {
        match out.iter_mut().find(|(k, _)| *k == t.kind) {
            Some((_, names)) => names.push(t.name.clone()),
            None => out.push((t.kind, vec![t.name.clone()])),
        }
    }
    out.sort_by_key(|(k, _)| *k);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Lawn, Plant};

    #[test]
    fn calendar_and_reminders() {
        let cat = Catalog::builtin();
        let today = NaiveDate::from_ymd_opt(2026, 11, 23).unwrap();
        let mut g = Garden::default();
        let mut p = Plant { id: "p1".into(), name: "Tomatera".into(), added: Some(today - Duration::days(10)), ..Default::default() };
        p.overrides.water_days = Some(3);
        p.overrides.harvest_days = Some(80);
        p.planted = Some(today - Duration::days(10));
        g.plants.push(p);
        g.lawns.push(Lawn { id: "l1".into(), name: "Prado".into(), mow_days: Some(15), last_mowed: Some(today), ..Default::default() });
        let title = |t: &Task| format!("{} {}", t.kind.id(), t.name);
        let ev = events(&g, cat, today, &title);
        let water = ev.iter().find(|e| e.title == "water Tomatera").unwrap();
        assert_eq!(water.rrule.as_deref(), Some("FREQ=DAILY;INTERVAL=3"));
        assert_eq!(water.link.as_deref(), Some("otherwise://mi-huerta/plant/p1"));
        assert!(ev.iter().any(|e| e.title == "mow Prado" && e.link.as_deref() == Some("otherwise://mi-huerta/lawn/l1")));
        let harvest = ev.iter().find(|e| e.title.starts_with("harvest")).unwrap();
        assert!(harvest.rrule.is_none());

        let due = due_now(&g, cat, today);
        assert_eq!(due, vec![(TaskKind::Water, vec!["Tomatera".to_string()])], "watering overdue; mowing and harvest later");

        let dir = std::env::temp_dir().join(format!("mi-huerta-feed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let root = Root::at(&dir);
        assert!(publish(&root, &g, cat, today, &title).unwrap() >= 3);
        assert!(dir.join("mi-huerta/events.json").exists());
        g.settings.calendar = false;
        publish(&root, &g, cat, today, &title).unwrap();
        assert!(!dir.join("mi-huerta/events.json").exists(), "turned off: cleared");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
