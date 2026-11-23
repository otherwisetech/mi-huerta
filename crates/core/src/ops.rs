//! Changes to the garden as data: what the app does on a click and what an AI agent may only
//! **propose** (the app shows a card; the change happens only if the user accepts it).
use crate::catalog::{Catalog, Place};
use crate::model::{Action, Garden, HarvestAmount, Id, JournalEntry, Plant, RainSource, Supply, SupplyKind, TaskKind, new_id};
use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mark {
    Done,
    Skip,
    Snooze,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    AddPlant {
        name: String,
        #[serde(default)]
        species: Option<String>,
        place: Place,
        #[serde(default)]
        planted: Option<NaiveDate>,
        #[serde(default)]
        notes: Option<String>,
    },
    MarkTask {
        task: TaskKind,
        target: Id,
        mark: Mark,
        /// Snooze for this many days (default 1).
        #[serde(default)]
        days: Option<u32>,
    },
    AddJournal {
        #[serde(default)]
        plant: Option<Id>,
        note: String,
        #[serde(default)]
        pests: Option<String>,
        #[serde(default)]
        harvest_amount: Option<f64>,
        #[serde(default)]
        harvest_unit: Option<String>,
        #[serde(default)]
        date: Option<NaiveDate>,
    },
    AddSupply {
        name: String,
        #[serde(default)]
        kind: Option<SupplyKind>,
        #[serde(default)]
        quantity: Option<f64>,
        #[serde(default)]
        unit: Option<String>,
        #[serde(default)]
        low_at: Option<f64>,
    },
    LogRain {
        #[serde(default)]
        date: Option<NaiveDate>,
        #[serde(default)]
        mm: Option<f64>,
    },
    SetInterval {
        target: Id,
        task: TaskKind,
        days: u32,
    },
}

impl Op {
    /// Whether it can be applied to `g` (checked when proposed and again when accepted).
    pub fn check(&self, g: &Garden, cat: &Catalog) -> Result<(), String> {
        let known = |id: &str| g.plant(id).is_some() || g.lawn(id).is_some();
        match self {
            Op::AddPlant { name, species, .. } => {
                if name.trim().is_empty() {
                    return Err("name is empty".into());
                }
                if let Some(s) = species.as_deref().filter(|s| !s.is_empty())
                    && cat.get(s).is_none()
                {
                    return Err(format!("unknown species {s:?}; leave it empty for a plant outside the catalogue"));
                }
            }
            Op::MarkTask { task, target, .. } => {
                if !known(target) {
                    return Err(format!("no plant or lawn {target:?}"));
                }
                if (*task == TaskKind::Mow) != g.lawn(target).is_some() {
                    return Err("mowing is for lawns; lawns are only mowed".into());
                }
            }
            Op::AddJournal { plant, note, harvest_amount, .. } => {
                if let Some(p) = plant
                    && !known(p)
                {
                    return Err(format!("no plant or lawn {p:?}"));
                }
                if note.trim().is_empty() && harvest_amount.is_none() {
                    return Err("an entry needs a note or a harvest".into());
                }
            }
            Op::AddSupply { name, quantity, .. } => {
                if name.trim().is_empty() {
                    return Err("name is empty".into());
                }
                if quantity.is_some_and(|q| q < 0.0) {
                    return Err("negative quantity".into());
                }
            }
            Op::LogRain { mm, .. } => {
                if mm.is_some_and(|m| !(0.0..=500.0).contains(&m)) {
                    return Err("rain must be 0–500 mm".into());
                }
            }
            Op::SetInterval { target, task, days } => {
                if g.plant(target).is_none() {
                    return Err(format!("no plant {target:?}"));
                }
                if *task == TaskKind::Mow || *days == 0 || *days > 730 {
                    return Err("days must be 1–730, for water / fertilize / prune / harvest".into());
                }
            }
        }
        Ok(())
    }

    /// Applies it; returns the id of what was created or changed.
    pub fn apply(&self, g: &mut Garden, cat: &Catalog, today: NaiveDate) -> Result<Id, String> {
        self.check(g, cat)?;
        Ok(match self.clone() {
            Op::AddPlant { name, species, place, planted, notes } => {
                let id = new_id("p");
                g.plants.push(Plant { id: id.clone(), name: name.trim().to_string(), species: species.unwrap_or_default(), place, planted, added: Some(today), notes: notes.unwrap_or_default(), ..Default::default() });
                id
            }
            Op::MarkTask { task, target, mark, days } => {
                let action = match mark {
                    Mark::Done => Action::Done,
                    Mark::Skip => Action::Skipped,
                    Mark::Snooze => Action::Snoozed { until: today + Duration::days(days.unwrap_or(1).clamp(1, 60) as i64) },
                };
                crate::schedule::record(g, task, &target, action, today);
                target
            }
            Op::AddJournal { plant, note, pests, harvest_amount, harvest_unit, date } => {
                let id = new_id("j");
                g.journal.push(JournalEntry { id: id.clone(), plant, date: Some(date.unwrap_or(today)), note, pests: pests.unwrap_or_default(), harvest: harvest_amount.map(|amount| HarvestAmount { amount, unit: harvest_unit.unwrap_or_default() }), ..Default::default() });
                id
            }
            Op::AddSupply { name, kind, quantity, unit, low_at } => {
                let id = new_id("s");
                g.supplies.push(Supply { id: id.clone(), name: name.trim().to_string(), kind: kind.unwrap_or_default(), quantity: quantity.unwrap_or(0.0), unit: unit.unwrap_or_default(), low_at, ..Default::default() });
                id
            }
            Op::LogRain { date, mm } => {
                let d = date.unwrap_or(today);
                g.log_rain(d, mm, RainSource::Manual);
                d.to_string()
            }
            Op::SetInterval { target, task, days } => {
                let p = g.plants.iter_mut().find(|p| p.id == target).ok_or("no such plant")?;
                let o = &mut p.overrides;
                match task {
                    TaskKind::Water => o.water_days = Some(days),
                    TaskKind::Fertilize => o.fertilize_days = Some(days),
                    TaskKind::Prune => o.prune_days = Some(days),
                    TaskKind::Harvest => o.harvest_days = Some(days),
                    TaskKind::Mow => return Err("lawns keep their own interval".into()),
                }
                target
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ops_check_then_apply() {
        let cat = Catalog::builtin();
        let mut g = Garden::default();
        let today = NaiveDate::from_ymd_opt(2026, 11, 23).unwrap();
        let bad: Op = serde_json::from_value(serde_json::json!({"op": "add_plant", "name": "X", "species": "unicornio", "place": "matera"})).unwrap();
        assert!(bad.apply(&mut g, cat, today).is_err());
        let add: Op = serde_json::from_value(serde_json::json!({"op": "add_plant", "name": "Mi planta", "place": "matera"})).unwrap();
        let id = add.apply(&mut g, cat, today).unwrap();
        assert_eq!(g.plants[0].added, Some(today));
        let snooze = Op::MarkTask { task: TaskKind::Water, target: id.clone(), mark: Mark::Snooze, days: Some(2) };
        snooze.apply(&mut g, cat, today).unwrap();
        assert_eq!(g.log[0].action, Action::Snoozed { until: today + Duration::days(2) });
        assert!(Op::MarkTask { task: TaskKind::Mow, target: id.clone(), mark: Mark::Done, days: None }.check(&g, cat).is_err());
        Op::SetInterval { target: id.clone(), task: TaskKind::Water, days: 4 }.apply(&mut g, cat, today).unwrap();
        assert_eq!(g.plants[0].overrides.water_days, Some(4));
        assert!(Op::AddJournal { plant: Some("nope".into()), note: "x".into(), pests: None, harvest_amount: None, harvest_unit: None, date: None }.check(&g, cat).is_err());
        Op::LogRain { date: None, mm: Some(8.0) }.apply(&mut g, cat, today).unwrap();
        assert_eq!(g.rain[0].mm, Some(8.0));
        assert!(Op::LogRain { date: None, mm: Some(-1.0) }.check(&g, cat).is_err());
    }
}
