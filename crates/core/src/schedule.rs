//! Care tasks (water, fertilise, prune, mow, harvest) from each plant's rules and the last time
//! each was done.
//!
//! - **Interval**: the user's override, else the catalogue's most specific rule for the
//!   plant's place, the garden's climate and the season ([`crate::catalog::rule_for`]). No
//!   interval → no task, and the plant shows in [`Agenda::missing`] with the catalogue's hint.
//! - **Anchor**: the last time it was done or skipped, else the day the plant was added to the
//!   app, else the planting date, else today. Due = anchor + interval.
//! - **Snooze**: a snooze recorded after the anchor moves the due day to its `until`.
//! - **Rain** (watering, plants not under a roof): rain since the last watering of at least
//!   the garden's threshold (or logged without a measure) counts as a watering; rain forecast
//!   on the due day or the next one postpones the task to that day.
//! - **Harvest**: once, from planting + the catalogue's (or user's) days, until it is done.
use crate::catalog::{Catalog, Place, Species, rule_for};
use crate::model::{Action, CareEvent, Garden, Id, Plant, RainSource, TaskKind};
use chrono::{Duration, NaiveDate};
use serde::Serialize;

/// Where a task's interval comes from.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "from", rename_all = "lowercase")]
pub enum Origin {
    /// The catalogue, citing `source` (an id of `data/sources.toml`).
    Catalog { source: String, reference: String },
    /// The user's own interval.
    User,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "rain", rename_all = "lowercase")]
pub enum RainNote {
    /// It rained (counted as a watering): the task moved to `date` + interval.
    Rained { date: NaiveDate, mm: Option<f64> },
    /// Rain is forecast: the task waits until that day.
    Forecast { date: NaiveDate, mm: f64 },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Task {
    pub kind: TaskKind,
    pub target: Id,
    pub name: String,
    /// Catalogue id, empty for custom plants and lawns.
    pub species: String,
    pub due: NaiveDate,
    /// The last day of a harvest window.
    pub until: Option<NaiveDate>,
    pub interval: Option<u32>,
    pub origin: Origin,
    pub rain: Option<RainNote>,
    pub snoozed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Bucket {
    Overdue,
    Today,
    Week,
    Later,
}

impl Task {
    pub fn bucket(&self, today: NaiveDate) -> Bucket {
        if self.due < today {
            Bucket::Overdue
        } else if self.due == today {
            Bucket::Today
        } else if self.due <= today + Duration::days(7) {
            Bucket::Week
        } else {
            Bucket::Later
        }
    }

    /// Stable per target, kind and due day (calendar ids, notification ids).
    pub fn key(&self) -> String {
        format!("{}-{}-{}", self.kind.id(), self.target, self.due)
    }
}

/// A plant or lawn for which a kind of care has no interval yet: the user sets one.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Missing {
    pub kind: TaskKind,
    pub target: Id,
    pub name: String,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Agenda {
    /// Sorted by due day, then kind, then name.
    pub tasks: Vec<Task>,
    pub missing: Vec<Missing>,
}

impl Agenda {
    pub fn in_bucket(&self, b: Bucket, today: NaiveDate) -> Vec<&Task> {
        self.tasks.iter().filter(|t| t.bucket(today) == b).collect()
    }

    /// Overdue, today and the next 7 days.
    pub fn this_week(&self, today: NaiveDate) -> Vec<&Task> {
        self.tasks.iter().filter(|t| t.bucket(today) != Bucket::Later).collect()
    }
}

/// The last done-or-skipped day of `kind` on `target`, and the last snooze with its day.
fn history(log: &[CareEvent], kind: TaskKind, target: &str) -> (Option<NaiveDate>, Option<(NaiveDate, NaiveDate)>) {
    let mut last: Option<NaiveDate> = None;
    let mut snooze: Option<(NaiveDate, NaiveDate)> = None;
    for e in log.iter().filter(|e| e.task == kind && e.target == target) {
        match &e.action {
            Action::Done | Action::Skipped => {
                if last.is_none_or(|l| e.date >= l) {
                    last = Some(e.date);
                }
            }
            Action::Snoozed { until } => {
                if snooze.is_none_or(|(d, _)| e.date >= d) {
                    snooze = Some((e.date, *until));
                }
            }
        }
    }
    (last, snooze)
}

fn interval(plant: &Plant, sp: Option<&Species>, kind: TaskKind, g: &Garden) -> Option<(u32, Origin)> {
    let user = match kind {
        TaskKind::Water => plant.overrides.water_days,
        TaskKind::Fertilize => plant.overrides.fertilize_days,
        TaskKind::Prune => plant.overrides.prune_days,
        TaskKind::Harvest => plant.overrides.harvest_days,
        TaskKind::Mow => None,
    };
    if let Some(d) = user.filter(|d| *d > 0) {
        return Some((d, Origin::User));
    }
    let sp = sp?;
    let (climate, season) = (g.settings.climate, g.settings.season);
    let cat = |days: u32, source: &str, reference: &str| Some((days, Origin::Catalog { source: source.to_string(), reference: reference.to_string() }));
    match kind {
        TaskKind::Water => rule_for(&sp.water, plant.place, climate, season).and_then(|r| cat(r.days, &r.source, &r.reference)),
        TaskKind::Fertilize => rule_for(&sp.fertilize, plant.place, climate, season).and_then(|r| cat(r.days, &r.source, &r.reference)),
        TaskKind::Prune => sp.prune.as_ref().and_then(|p| cat(p.days, &p.source, &p.reference)),
        TaskKind::Harvest => sp.harvest.as_ref().and_then(|h| cat(h.from_days, &h.source, &h.reference)),
        TaskKind::Mow => None,
    }
}

fn hint(sp: Option<&Species>, kind: TaskKind) -> Option<String> {
    let h = &sp?.hints;
    match kind {
        TaskKind::Water => h.water.clone(),
        TaskKind::Fertilize => h.fertilize.clone(),
        TaskKind::Prune => h.prune.clone(),
        TaskKind::Harvest => h.harvest.clone(),
        TaskKind::Mow => h.mow.clone(),
    }
}

/// Counts toward the rain threshold.
fn enough(mm: Option<f64>, threshold: f64) -> bool {
    mm.is_none_or(|m| m >= threshold)
}

/// The latest rain that counts as a watering after `anchor`, up to `today`.
fn rained_since(g: &Garden, anchor: NaiveDate, today: NaiveDate) -> Option<(NaiveDate, Option<f64>)> {
    let t = g.settings.weather.skip_mm;
    g.rain
        .iter()
        // forecasts only count when the weather is on; manual entries always do
        .filter(|r| r.source == RainSource::Manual || g.settings.weather.enabled)
        .filter(|r| r.date > anchor && r.date <= today && enough(r.mm, t))
        .max_by_key(|r| r.date)
        .map(|r| (r.date, r.mm))
}

fn forecast_rain(g: &Garden, due: NaiveDate, today: NaiveDate) -> Option<(NaiveDate, f64)> {
    if !g.settings.weather.enabled {
        return None;
    }
    let t = g.settings.weather.skip_mm;
    let f = g.forecast.as_ref()?;
    let from = due.max(today);
    f.days.iter().filter(|d| d.date >= from && d.date <= from + Duration::days(1) && d.rain_mm >= t).min_by_key(|d| d.date).map(|d| (d.date, d.rain_mm))
}

/// Every upcoming task of the garden on `today`.
pub fn agenda(g: &Garden, cat: &Catalog, today: NaiveDate) -> Agenda {
    let mut a = Agenda::default();
    for p in g.plants.iter().filter(|p| !p.archived) {
        let sp = cat.get(&p.species);
        for kind in [TaskKind::Water, TaskKind::Fertilize, TaskKind::Prune, TaskKind::Harvest] {
            if p.place == Place::Prado && kind == TaskKind::Harvest {
                continue;
            }
            let Some((days, origin)) = interval(p, sp, kind, g) else {
                a.missing.push(Missing { kind, target: p.id.clone(), name: p.name.clone(), hint: hint(sp, kind) });
                continue;
            };
            let (last, snooze) = history(&g.log, kind, &p.id);
            let mut task = Task { kind, target: p.id.clone(), name: p.name.clone(), species: p.species.clone(), due: today, until: None, interval: Some(days), origin, rain: None, snoozed: false };
            if kind == TaskKind::Harvest {
                // once per planting: a harvest done or skipped after planting ends it
                let Some(planted) = p.planted else {
                    a.missing.push(Missing { kind, target: p.id.clone(), name: p.name.clone(), hint: hint(sp, kind) });
                    continue;
                };
                if last.is_some_and(|l| l >= planted) {
                    continue;
                }
                task.due = planted + Duration::days(days as i64);
                let to = if matches!(task.origin, Origin::User) { None } else { sp.and_then(|s| s.harvest.as_ref()).and_then(|h| h.to_days) };
                task.until = to.map(|t| planted + Duration::days(t as i64));
                task.interval = None;
            } else {
                let mut anchor = last.or(p.added).or(p.planted).unwrap_or(today);
                if kind == TaskKind::Water
                    && !p.covered
                    && let Some((date, mm)) = rained_since(g, anchor, today)
                {
                    anchor = date;
                    task.rain = Some(RainNote::Rained { date, mm });
                }
                task.due = anchor + Duration::days(days as i64);
                if kind == TaskKind::Water
                    && !p.covered
                    && let Some((date, mm)) = forecast_rain(g, task.due, today)
                {
                    task.due = date;
                    task.rain = Some(RainNote::Forecast { date, mm });
                }
            }
            if let Some((at, until)) = snooze
                && last.is_none_or(|l| at >= l)
                && until > task.due
            {
                task.due = until;
                task.snoozed = true;
            }
            a.tasks.push(task);
        }
    }
    let lawn_days = cat.plants.iter().find(|s| s.kind == crate::catalog::Kind::Prado).and_then(|s| s.mow.as_ref()).and_then(|m| m.days.map(|d| (d, Origin::Catalog { source: m.source.clone(), reference: m.reference.clone() })));
    let lawn_hint = cat.plants.iter().find(|s| s.kind == crate::catalog::Kind::Prado).and_then(|s| s.hints.mow.clone());
    for l in &g.lawns {
        let Some((days, origin)) = l.mow_days.filter(|d| *d > 0).map(|d| (d, Origin::User)).or_else(|| lawn_days.clone()) else {
            a.missing.push(Missing { kind: TaskKind::Mow, target: l.id.clone(), name: l.name.clone(), hint: lawn_hint.clone() });
            continue;
        };
        let (last, snooze) = history(&g.log, TaskKind::Mow, &l.id);
        let anchor = match (last, l.last_mowed) {
            (Some(a), Some(b)) => a.max(b),
            (a, b) => a.or(b).unwrap_or(today),
        };
        let mut task = Task { kind: TaskKind::Mow, target: l.id.clone(), name: l.name.clone(), species: String::new(), due: anchor + Duration::days(days as i64), until: None, interval: Some(days), origin, rain: None, snoozed: false };
        if let Some((at, until)) = snooze
            && at >= anchor
            && until > task.due
        {
            task.due = until;
            task.snoozed = true;
        }
        a.tasks.push(task);
    }
    a.tasks.sort_by(|x, y| x.due.cmp(&y.due).then(x.kind.cmp(&y.kind)).then(x.name.cmp(&y.name)));
    a
}

/// Done, skipped (both reset the count) or snoozed, on `today`. Mowing done also updates the
/// lawn's last mowing day.
pub fn record(g: &mut Garden, kind: TaskKind, target: &str, action: Action, today: NaiveDate) {
    if kind == TaskKind::Mow
        && action == Action::Done
        && let Some(l) = g.lawns.iter_mut().find(|l| l.id == target)
    {
        l.last_mowed = Some(today);
    }
    g.log.push(CareEvent { task: kind, target: target.to_string(), date: today, action, note: String::new() });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{Climate, Season};
    use crate::model::{Forecast, ForecastDay, Lawn, Overrides};

    fn d(m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, m, day).unwrap()
    }

    /// A synthetic catalogue: watering every 7 days, every 3 in a pot, every 2 in a pot in a
    /// warm climate's dry season; fertilising every 30; harvest 60–90 days after sowing.
    fn catalog() -> Catalog {
        let cat = r#"
schema = 1
[[plant]]
id = "mata"
kind = "hortaliza"
scientific = "Planta ficticia"
es = "Mata"
[[plant.water]]
days = 7
source = "s"
ref = "p. 1"
[[plant.water]]
place = "matera"
days = 3
source = "s"
ref = "p. 1"
[[plant.water]]
place = "matera"
climate = "calido"
season = "seca"
days = 2
source = "s"
ref = "p. 1"
[[plant.fertilize]]
days = 30
source = "s"
ref = "p. 2"
[plant.harvest]
from_days = 60
to_days = 90
source = "s"
ref = "p. 3"
[plant.hints]
prune = "Quita las hojas secas."

[[plant]]
id = "pasto"
kind = "prado"
scientific = "Pasto ficticio"
es = "Prado"
[plant.mow]
days = 10
source = "s"
ref = "p. 4"
[plant.hints]
water = "a"
fertilize = "b"
prune = "c"
harvest = "d"
"#;
        let src = "schema = 1\n[[source]]\nid = \"s\"\npublisher = \"P\"\ntitle = \"T\"\nurl = \"https://example.org\"\n";
        let c = Catalog::parse(cat, src).unwrap();
        assert!(c.problems().is_empty(), "{:?}", c.problems());
        c
    }

    fn garden(place: Place) -> Garden {
        let mut g = Garden::default();
        g.plants.push(Plant { id: "p1".into(), name: "Mata del patio".into(), species: "mata".into(), place, planted: Some(d(9, 1)), added: Some(d(11, 1)), ..Default::default() });
        g
    }

    fn task(a: &Agenda, kind: TaskKind) -> Option<&Task> {
        a.tasks.iter().find(|t| t.kind == kind)
    }

    #[test]
    fn intervals_follow_place_climate_season_and_overrides() {
        let cat = catalog();
        let mut g = garden(Place::Huerta);
        let today = d(11, 5);
        assert_eq!(task(&agenda(&g, &cat, today), TaskKind::Water).unwrap().due, d(11, 8), "added + 7");
        g.plants[0].place = Place::Matera;
        assert_eq!(task(&agenda(&g, &cat, today), TaskKind::Water).unwrap().due, d(11, 4));
        g.settings.climate = Some(Climate::Calido);
        let a = agenda(&g, &cat, today);
        let w = task(&a, TaskKind::Water).unwrap();
        assert_eq!((w.due, w.interval), (d(11, 3), Some(2)));
        assert_eq!(w.bucket(today), Bucket::Overdue);
        assert!(matches!(&w.origin, Origin::Catalog { source, .. } if source == "s"));
        g.settings.season = Season::Lluviosa;
        assert_eq!(task(&agenda(&g, &cat, today), TaskKind::Water).unwrap().due, d(11, 4));
        g.plants[0].overrides = Overrides { water_days: Some(5), ..Default::default() };
        let a = agenda(&g, &cat, today);
        assert_eq!(task(&a, TaskKind::Water).unwrap().due, d(11, 6));
        assert_eq!(task(&a, TaskKind::Water).unwrap().origin, Origin::User);
        assert_eq!(task(&a, TaskKind::Fertilize).unwrap().due, d(12, 1));
        // no prune interval: the hint instead of a task
        assert!(task(&a, TaskKind::Prune).is_none());
        let m = a.missing.iter().find(|m| m.kind == TaskKind::Prune).unwrap();
        assert_eq!(m.hint.as_deref(), Some("Quita las hojas secas."));
    }

    #[test]
    fn done_skip_and_snooze() {
        let cat = catalog();
        let mut g = garden(Place::Huerta);
        record(&mut g, TaskKind::Water, "p1", Action::Done, d(11, 10));
        assert_eq!(task(&agenda(&g, &cat, d(11, 10)), TaskKind::Water).unwrap().due, d(11, 17));
        record(&mut g, TaskKind::Water, "p1", Action::Snoozed { until: d(11, 20) }, d(11, 17));
        let t = agenda(&g, &cat, d(11, 17));
        let w = task(&t, TaskKind::Water).unwrap();
        assert_eq!((w.due, w.snoozed), (d(11, 20), true));
        record(&mut g, TaskKind::Water, "p1", Action::Skipped, d(11, 20));
        let w = agenda(&g, &cat, d(11, 20));
        let w = task(&w, TaskKind::Water).unwrap();
        assert_eq!((w.due, w.snoozed), (d(11, 27), false), "a skip resets the count and ends the snooze");
    }

    #[test]
    fn rain_counts_as_watering_and_forecast_postpones() {
        let cat = catalog();
        let mut g = garden(Place::Huerta);
        record(&mut g, TaskKind::Water, "p1", Action::Done, d(11, 1));
        // manual rain without a measure counts, even with weather off
        g.log_rain(d(11, 4), None, RainSource::Manual);
        let a = agenda(&g, &cat, d(11, 6));
        let w = task(&a, TaskKind::Water).unwrap();
        assert_eq!(w.due, d(11, 11));
        assert_eq!(w.rain, Some(RainNote::Rained { date: d(11, 4), mm: None }));
        // light rain under the threshold doesn't
        g.rain.clear();
        g.log_rain(d(11, 4), Some(1.5), RainSource::Manual);
        assert_eq!(task(&agenda(&g, &cat, d(11, 6)), TaskKind::Water).unwrap().due, d(11, 8));
        // under a roof, rain never counts
        g.log_rain(d(11, 5), Some(20.0), RainSource::Manual);
        g.plants[0].covered = true;
        assert_eq!(task(&agenda(&g, &cat, d(11, 6)), TaskKind::Water).unwrap().due, d(11, 8));
        g.plants[0].covered = false;
        assert_eq!(task(&agenda(&g, &cat, d(11, 6)), TaskKind::Water).unwrap().due, d(11, 12));
        // forecast: only with weather on
        g.rain.clear();
        g.forecast = Some(Forecast { fetched_at: "2026-11-07T06:00:00Z".into(), days: vec![ForecastDay { date: d(11, 8), rain_mm: 0.4, probability: None }, ForecastDay { date: d(11, 9), rain_mm: 12.0, probability: Some(80.0) }] });
        assert_eq!(task(&agenda(&g, &cat, d(11, 7)), TaskKind::Water).unwrap().due, d(11, 8));
        g.settings.weather.enabled = true;
        let a = agenda(&g, &cat, d(11, 7));
        let w = task(&a, TaskKind::Water).unwrap();
        assert_eq!(w.due, d(11, 9), "postponed to the rainy day");
        assert_eq!(w.rain, Some(RainNote::Forecast { date: d(11, 9), mm: 12.0 }));
        // Open-Meteo's observed rain counts only with weather on
        g.forecast = None;
        g.log_rain(d(11, 7), Some(9.0), RainSource::OpenMeteo);
        assert_eq!(task(&agenda(&g, &cat, d(11, 7)), TaskKind::Water).unwrap().due, d(11, 14));
        g.settings.weather.enabled = false;
        assert_eq!(task(&agenda(&g, &cat, d(11, 7)), TaskKind::Water).unwrap().due, d(11, 8));
    }

    #[test]
    fn harvest_once_and_lawns() {
        let cat = catalog();
        let mut g = garden(Place::Huerta);
        let a = agenda(&g, &cat, d(11, 5));
        let h = task(&a, TaskKind::Harvest).unwrap();
        assert_eq!((h.due, h.until), (d(10, 31), Some(d(11, 30))));
        record(&mut g, TaskKind::Harvest, "p1", Action::Done, d(11, 5));
        assert!(task(&agenda(&g, &cat, d(11, 6)), TaskKind::Harvest).is_none());
        g.plants[0].planted = None;
        assert!(agenda(&g, &cat, d(11, 6)).missing.iter().any(|m| m.kind == TaskKind::Harvest));

        g.lawns.push(Lawn { id: "l1".into(), name: "Prado de enfrente".into(), last_mowed: Some(d(11, 1)), ..Default::default() });
        let a = agenda(&g, &cat, d(11, 6));
        let m = task(&a, TaskKind::Mow).unwrap();
        assert_eq!((m.due, m.interval), (d(11, 11), Some(10)), "the catalogue's lawn interval");
        g.lawns[0].mow_days = Some(14);
        record(&mut g, TaskKind::Mow, "l1", Action::Done, d(11, 12));
        assert_eq!(g.lawns[0].last_mowed, Some(d(11, 12)));
        assert_eq!(task(&agenda(&g, &cat, d(11, 12)), TaskKind::Mow).unwrap().due, d(11, 26));
    }

    #[test]
    fn buckets_and_custom_plants() {
        let cat = catalog();
        let mut g = garden(Place::Huerta);
        g.plants.push(Plant { id: "p2".into(), name: "Mi planta rara".into(), added: Some(d(11, 1)), overrides: Overrides { water_days: Some(4), ..Default::default() }, ..Default::default() });
        let today = d(11, 5);
        let a = agenda(&g, &cat, today);
        let rara: Vec<&Task> = a.tasks.iter().filter(|t| t.target == "p2").collect();
        assert_eq!(rara.len(), 1);
        assert_eq!(rara[0].bucket(today), Bucket::Today);
        assert!(a.missing.iter().filter(|m| m.target == "p2").count() == 3, "fertilise, prune, harvest have no interval");
        assert!(a.this_week(today).iter().all(|t| t.due <= today + Duration::days(7)));
        assert!(a.tasks.windows(2).all(|w| w[0].due <= w[1].due));
        g.plants[1].archived = true;
        assert!(agenda(&g, &cat, today).tasks.iter().all(|t| t.target != "p2"));
    }
}
