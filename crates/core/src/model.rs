//! The garden: plants, lawns, supplies and purchases, the journal, what was done (care log),
//! rain, settings, and the sensor extension point. Stored as one JSON file (`garden.json`)
//! in the garden folder; see [`crate::store`].
//!
//! Forward compatibility: fields this version doesn't know are kept (`extra`) and written back.
use crate::catalog::{Climate, Place, Season};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const SCHEMA: u32 = 1;

pub type Id = String;

pub fn new_id(prefix: &str) -> Id {
    format!("{prefix}-{}", &uuid::Uuid::new_v4().simple().to_string()[..10])
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Garden {
    pub schema: u32,
    /// Bumped on every save; a stale save is refused (another process wrote meanwhile).
    pub revision: u64,
    pub settings: Settings,
    pub plants: Vec<Plant>,
    pub lawns: Vec<Lawn>,
    pub supplies: Vec<Supply>,
    pub purchases: Vec<Purchase>,
    pub journal: Vec<JournalEntry>,
    pub log: Vec<CareEvent>,
    pub rain: Vec<RainEntry>,
    /// The last forecast fetched (only with weather on).
    pub forecast: Option<Forecast>,
    /// Extension point for v0.2 (docs/sensors.md): nothing in v0.1 reads or writes these.
    pub sensors: Vec<Sensor>,
    pub readings: Vec<Reading>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Garden {
    fn default() -> Self {
        Garden { schema: SCHEMA, revision: 0, settings: Settings::default(), plants: vec![], lawns: vec![], supplies: vec![], purchases: vec![], journal: vec![], log: vec![], rain: vec![], forecast: None, sensors: vec![], readings: vec![], extra: Map::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The garden's thermal floor; `None` until the user says.
    pub climate: Option<Climate>,
    /// The season now (the user switches it; Colombia has dry and rainy seasons).
    pub season: Season,
    pub weather: Weather,
    /// Publish care tasks to Otherwise Apps' calendar (`<data root>/mi-huerta/events.json`).
    pub calendar: bool,
    /// Daily "Hoy toca…" notifications through Otherwise Helper.
    pub notifications: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { climate: None, season: Season::Seca, weather: Weather::default(), calendar: true, notifications: true, extra: Map::new() }
    }
}

/// Weather is opt-in: off by default; only a coarse location is ever sent (to Open-Meteo).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Weather {
    pub enabled: bool,
    /// What the user typed (a city), shown back to them; never sent anywhere by itself.
    pub place: String,
    /// Rounded to [`crate::weather::COORD_DECIMALS`] before saving and before any request.
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    /// Rain (mm) from which a watering counts as done by the rain. App default, adjustable.
    pub skip_mm: f64,
}

impl Default for Weather {
    fn default() -> Self {
        Weather { enabled: false, place: String::new(), lat: None, lon: None, skip_mm: 5.0 }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Overrides {
    pub water_days: Option<u32>,
    pub fertilize_days: Option<u32>,
    pub prune_days: Option<u32>,
    pub harvest_days: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Plant {
    pub id: Id,
    pub name: String,
    /// Catalogue id (`tomate`), or empty for a plant the catalogue doesn't have.
    pub species: String,
    pub place: Place,
    pub planted: Option<NaiveDate>,
    /// When it was added to the app (the first care tasks count from here without `planted`).
    pub added: Option<NaiveDate>,
    /// Under a roof: rain doesn't water it.
    pub covered: bool,
    /// Paths relative to the garden folder (`photos/…`).
    pub photos: Vec<String>,
    pub notes: String,
    /// The user's own intervals; they win over the catalogue.
    pub overrides: Overrides,
    pub archived: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Plant {
    fn default() -> Self {
        Plant { id: String::new(), name: String::new(), species: String::new(), place: Place::Huerta, planted: None, added: None, covered: false, photos: vec![], notes: String::new(), overrides: Overrides::default(), archived: false, extra: Map::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Lawn {
    pub id: Id,
    pub name: String,
    pub area_m2: Option<f64>,
    /// The user's interval; else the catalogue's `prado` entry, if it has one.
    pub mow_days: Option<u32>,
    pub height_cm: Option<f64>,
    pub last_mowed: Option<NaiveDate>,
    pub notes: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SupplyKind {
    #[default]
    Abono,
    Tierra,
    Semillas,
    Insecticida,
    Herramienta,
    Otro,
}

impl SupplyKind {
    pub const ALL: [SupplyKind; 6] = [SupplyKind::Abono, SupplyKind::Tierra, SupplyKind::Semillas, SupplyKind::Insecticida, SupplyKind::Herramienta, SupplyKind::Otro];
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Supply {
    pub id: Id,
    pub name: String,
    pub kind: SupplyKind,
    /// "kg", "L", "sobres"… free text.
    pub unit: String,
    pub quantity: f64,
    /// "Running low" at or below this.
    pub low_at: Option<f64>,
    pub notes: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Supply {
    pub fn is_low(&self) -> bool {
        self.low_at.is_some_and(|l| self.quantity <= l)
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Purchase {
    pub id: Id,
    pub supply: Id,
    pub date: Option<NaiveDate>,
    pub quantity: f64,
    /// Total paid, in `currency` units (COP by default).
    pub cost: f64,
    pub currency: String,
    pub shop: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HarvestAmount {
    pub amount: f64,
    /// "kg", "unidades", "manojos"…
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct JournalEntry {
    pub id: Id,
    /// The plant (or lawn) it is about; `None`: the whole garden.
    pub plant: Option<Id>,
    pub date: Option<NaiveDate>,
    pub note: String,
    pub photos: Vec<String>,
    pub pests: String,
    pub harvest: Option<HarvestAmount>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskKind {
    Water,
    Fertilize,
    Prune,
    Mow,
    Harvest,
}

impl TaskKind {
    pub const ALL: [TaskKind; 5] = [TaskKind::Water, TaskKind::Fertilize, TaskKind::Prune, TaskKind::Mow, TaskKind::Harvest];

    pub fn id(self) -> &'static str {
        match self {
            TaskKind::Water => "water",
            TaskKind::Fertilize => "fertilize",
            TaskKind::Prune => "prune",
            TaskKind::Mow => "mow",
            TaskKind::Harvest => "harvest",
        }
    }

    pub fn parse(s: &str) -> Option<TaskKind> {
        TaskKind::ALL.into_iter().find(|k| k.id() == s)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum Action {
    Done,
    Skipped,
    /// Not before `until`.
    Snoozed { until: NaiveDate },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CareEvent {
    pub task: TaskKind,
    /// Plant or lawn id.
    pub target: Id,
    pub date: NaiveDate,
    #[serde(flatten)]
    pub action: Action,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RainSource {
    #[default]
    Manual,
    OpenMeteo,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RainEntry {
    pub date: NaiveDate,
    /// `None`: "it rained" without a measure (counts as enough).
    #[serde(default)]
    pub mm: Option<f64>,
    #[serde(default)]
    pub source: RainSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForecastDay {
    pub date: NaiveDate,
    pub rain_mm: f64,
    #[serde(default)]
    pub probability: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Forecast {
    /// RFC 3339.
    pub fetched_at: String,
    pub days: Vec<ForecastDay>,
}

/// v0.2 extension point (docs/sensors.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensorKind {
    SoilMoisture,
    Temperature,
    TankLevel,
    Valve,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sensor {
    pub id: Id,
    pub kind: SensorKind,
    pub name: String,
    /// The plant it measures, or a zone name (a bed, the tank).
    #[serde(default)]
    pub plant: Option<Id>,
    #[serde(default)]
    pub zone: Option<String>,
    /// The device in Nodo (`otherwise://nodo/device/<id>`).
    #[serde(default)]
    pub nodo_device: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reading {
    pub sensor: Id,
    /// RFC 3339.
    pub at: String,
    pub value: f64,
    /// "%", "°C", "L", "open"/"closed" as 1/0.
    pub unit: String,
}

impl Garden {
    pub fn plant(&self, id: &str) -> Option<&Plant> {
        self.plants.iter().find(|p| p.id == id)
    }

    pub fn lawn(&self, id: &str) -> Option<&Lawn> {
        self.lawns.iter().find(|l| l.id == id)
    }

    pub fn supply(&self, id: &str) -> Option<&Supply> {
        self.supplies.iter().find(|s| s.id == id)
    }

    /// The name of a plant or lawn.
    pub fn target_name(&self, id: &str) -> Option<&str> {
        self.plant(id).map(|p| p.name.as_str()).or_else(|| self.lawn(id).map(|l| l.name.as_str()))
    }

    /// Records a purchase and adds its quantity to the supply.
    pub fn add_purchase(&mut self, p: Purchase) {
        if let Some(s) = self.supplies.iter_mut().find(|s| s.id == p.supply) {
            s.quantity += p.quantity;
        }
        self.purchases.push(p);
    }

    pub fn low_supplies(&self) -> Vec<&Supply> {
        self.supplies.iter().filter(|s| s.is_low()).collect()
    }

    /// Spent on a supply (all purchases), per currency.
    pub fn spent(&self) -> Vec<(String, f64)> {
        let mut by: std::collections::BTreeMap<String, f64> = Default::default();
        for p in &self.purchases {
            *by.entry(if p.currency.is_empty() { "COP".into() } else { p.currency.clone() }).or_default() += p.cost;
        }
        by.into_iter().collect()
    }

    /// Records rain on `date` (replacing an entry of the same source that day).
    pub fn log_rain(&mut self, date: NaiveDate, mm: Option<f64>, source: RainSource) {
        self.rain.retain(|r| !(r.date == date && r.source == source));
        self.rain.push(RainEntry { date, mm, source });
        self.rain.sort_by_key(|r| r.date);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_fields_survive_a_round_trip() {
        let raw = r#"{"schema":1,"plants":[{"id":"p1","name":"Tomatera","species":"tomate","place":"matera","futureField":{"a":1}}],"newTopLevel":[1,2]}"#;
        let g: Garden = serde_json::from_str(raw).unwrap();
        assert_eq!(g.plants[0].place, Place::Matera);
        let back = serde_json::to_value(&g).unwrap();
        assert_eq!(back["newTopLevel"], serde_json::json!([1, 2]));
        assert_eq!(back["plants"][0]["futureField"]["a"], 1);
        assert!(!g.settings.weather.enabled, "weather is off by default");
    }

    #[test]
    fn purchases_restock_and_low_supplies() {
        let mut g = Garden::default();
        g.supplies.push(Supply { id: "s1".into(), name: "Compost".into(), unit: "kg".into(), quantity: 1.0, low_at: Some(2.0), ..Default::default() });
        assert_eq!(g.low_supplies().len(), 1);
        g.add_purchase(Purchase { id: "b1".into(), supply: "s1".into(), quantity: 5.0, cost: 18000.0, currency: "COP".into(), ..Default::default() });
        assert!(g.low_supplies().is_empty());
        assert_eq!(g.spent(), vec![("COP".to_string(), 18000.0)]);
    }

    #[test]
    fn care_events_serialize_flat() {
        let e = CareEvent { task: TaskKind::Water, target: "p1".into(), date: NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(), action: Action::Snoozed { until: NaiveDate::from_ymd_opt(2026, 11, 3).unwrap() }, note: String::new() };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["action"], "snoozed");
        assert_eq!(v["until"], "2026-11-03");
        assert_eq!(serde_json::from_value::<CareEvent>(v).unwrap(), e);
    }
}
