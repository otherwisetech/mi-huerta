//! The built-in plant catalogue (`data/catalog.toml`) and its sources (`data/sources.toml`),
//! embedded at build time.
//!
//! Every number in the catalogue cites a source (`source` + `ref`); where no source was found
//! the value is left out and a `hint` tells the user how to decide. [`Catalog::problems`]
//! checks both rules and the test suite fails on any problem.
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub const CATALOG_TOML: &str = include_str!("../../../data/catalog.toml");
pub const SOURCES_TOML: &str = include_str!("../../../data/sources.toml");

/// Where a plant lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Place {
    Matera,
    Huerta,
    Jardin,
    Prado,
}

impl Place {
    pub const ALL: [Place; 4] = [Place::Matera, Place::Huerta, Place::Jardin, Place::Prado];

    pub fn id(self) -> &'static str {
        match self {
            Place::Matera => "matera",
            Place::Huerta => "huerta",
            Place::Jardin => "jardin",
            Place::Prado => "prado",
        }
    }
}

/// Colombian thermal floors (pisos térmicos).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Climate {
    Calido,
    Templado,
    Frio,
}

impl Climate {
    pub const ALL: [Climate; 3] = [Climate::Calido, Climate::Templado, Climate::Frio];

    pub fn id(self) -> &'static str {
        match self {
            Climate::Calido => "calido",
            Climate::Templado => "templado",
            Climate::Frio => "frio",
        }
    }
}

/// Colombia has dry and rainy seasons rather than four.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Season {
    #[default]
    Seca,
    Lluviosa,
}

impl Season {
    pub fn id(self) -> &'static str {
        match self {
            Season::Seca => "seca",
            Season::Lluviosa => "lluviosa",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Hortaliza,
    Aromatica,
    Medicinal,
    Frutal,
    Ornamental,
    Prado,
}

impl Kind {
    pub fn id(self) -> &'static str {
        match self {
            Kind::Hortaliza => "hortaliza",
            Kind::Aromatica => "aromatica",
            Kind::Medicinal => "medicinal",
            Kind::Frutal => "frutal",
            Kind::Ornamental => "ornamental",
            Kind::Prado => "prado",
        }
    }
}

/// A sourced interval in days, optionally only for a place, climate or season.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    #[serde(default)]
    pub place: Option<Place>,
    #[serde(default)]
    pub climate: Option<Climate>,
    #[serde(default)]
    pub season: Option<Season>,
    pub days: u32,
    /// What to apply (fertiliser rules), free text in Spanish.
    #[serde(default)]
    pub product: Option<String>,
    #[serde(default)]
    pub source: String,
    #[serde(default, rename = "ref")]
    pub reference: String,
}

impl Rule {
    fn matches(&self, place: Place, climate: Option<Climate>, season: Season) -> bool {
        self.place.is_none_or(|p| p == place) && self.climate.is_none_or(|c| Some(c) == climate) && self.season.is_none_or(|s| s == season)
    }

    fn specificity(&self) -> u8 {
        self.place.is_some() as u8 * 4 + self.climate.is_some() as u8 * 2 + self.season.is_some() as u8
    }
}

/// The most specific rule that applies (place weighs more than climate, climate more than
/// season); the first one listed on a tie.
pub fn rule_for(rules: &[Rule], place: Place, climate: Option<Climate>, season: Season) -> Option<&Rule> {
    let mut best: Option<&Rule> = None;
    for r in rules.iter().filter(|r| r.matches(place, climate, season)) {
        if best.is_none_or(|b| r.specificity() > b.specificity()) {
            best = Some(r);
        }
    }
    best
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Interval {
    pub days: u32,
    #[serde(default)]
    pub source: String,
    #[serde(default, rename = "ref")]
    pub reference: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HarvestFrom {
    #[default]
    Siembra,
    Trasplante,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Harvest {
    pub from_days: u32,
    #[serde(default)]
    pub to_days: Option<u32>,
    #[serde(default)]
    pub from: HarvestFrom,
    #[serde(default)]
    pub source: String,
    #[serde(default, rename = "ref")]
    pub reference: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mow {
    #[serde(default)]
    pub days: Option<u32>,
    #[serde(default)]
    pub height_min_cm: Option<f64>,
    #[serde(default)]
    pub height_max_cm: Option<f64>,
    #[serde(default)]
    pub source: String,
    #[serde(default, rename = "ref")]
    pub reference: String,
}

/// Plain-Spanish advice where the catalogue has no sourced number (or extra advice).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Hints {
    #[serde(default)]
    pub water: Option<String>,
    #[serde(default)]
    pub fertilize: Option<String>,
    #[serde(default)]
    pub prune: Option<String>,
    #[serde(default)]
    pub harvest: Option<String>,
    #[serde(default)]
    pub mow: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Species {
    pub id: String,
    pub kind: Kind,
    pub scientific: String,
    pub es: String,
    #[serde(default)]
    pub en: String,
    #[serde(default)]
    pub pt: String,
    #[serde(default)]
    pub fr: String,
    #[serde(default)]
    pub places: Vec<Place>,
    #[serde(default)]
    pub climates: Vec<Climate>,
    #[serde(default)]
    pub climates_source: Option<String>,
    #[serde(default)]
    pub climates_ref: Option<String>,
    #[serde(default)]
    pub water: Vec<Rule>,
    #[serde(default)]
    pub fertilize: Vec<Rule>,
    #[serde(default)]
    pub prune: Option<Interval>,
    #[serde(default)]
    pub harvest: Option<Harvest>,
    #[serde(default)]
    pub mow: Option<Mow>,
    #[serde(default)]
    pub hints: Hints,
}

impl Species {
    /// The common name in `lang` (Spanish when there is none).
    pub fn name(&self, lang: &str) -> &str {
        let n = match lang {
            "en" => &self.en,
            "pt" => &self.pt,
            "fr" => &self.fr,
            _ => &self.es,
        };
        if n.trim().is_empty() { &self.es } else { n }
    }

    /// Every source id this entry cites.
    pub fn cited(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.water.iter().chain(&self.fertilize).map(|r| r.source.as_str()).collect();
        v.extend(self.prune.iter().map(|p| p.source.as_str()));
        v.extend(self.harvest.iter().map(|h| h.source.as_str()));
        v.extend(self.mow.iter().map(|m| m.source.as_str()));
        v.extend(self.climates_source.as_deref());
        v.sort_unstable();
        v.dedup();
        v
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Source {
    pub id: String,
    pub publisher: String,
    pub title: String,
    #[serde(default)]
    pub year: Option<i32>,
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct CatalogFile {
    #[serde(default)]
    schema: u32,
    #[serde(default)]
    climates: Option<toml::Value>,
    #[serde(default)]
    plant: Vec<Species>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct SourcesFile {
    #[serde(default)]
    schema: u32,
    #[serde(default)]
    source: Vec<Source>,
}

pub const SCHEMA: u32 = 1;

#[derive(Debug, Clone)]
pub struct Catalog {
    pub plants: Vec<Species>,
    pub sources: Vec<Source>,
    /// The thermal-floor table as written (altitudes, temperatures, its source).
    pub climates: Option<toml::Value>,
}

impl Catalog {
    pub fn parse(catalog: &str, sources: &str) -> Result<Catalog, String> {
        let c: CatalogFile = toml::from_str(catalog).map_err(|e| format!("catalog.toml: {e}"))?;
        let s: SourcesFile = toml::from_str(sources).map_err(|e| format!("sources.toml: {e}"))?;
        if c.schema > SCHEMA || s.schema > SCHEMA {
            return Err(format!("catalogue schema newer than {SCHEMA}"));
        }
        Ok(Catalog { plants: c.plant, sources: s.source, climates: c.climates })
    }

    /// The catalogue shipped with the app.
    pub fn builtin() -> &'static Catalog {
        static C: OnceLock<Catalog> = OnceLock::new();
        C.get_or_init(|| Catalog::parse(CATALOG_TOML, SOURCES_TOML).unwrap_or_else(|e| panic!("built-in catalogue: {e}")))
    }

    pub fn get(&self, id: &str) -> Option<&Species> {
        self.plants.iter().find(|p| p.id == id)
    }

    pub fn source(&self, id: &str) -> Option<&Source> {
        self.sources.iter().find(|s| s.id == id)
    }

    /// Everything that breaks the catalogue's rules: numbers without a known source or a
    /// reference, topics with neither a number nor a hint, duplicate ids.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut ids = std::collections::BTreeSet::new();
        for s in &self.sources {
            if !ids.insert(format!("source:{}", s.id)) {
                out.push(format!("duplicate source {}", s.id));
            }
            if s.url.trim().is_empty() || s.title.trim().is_empty() {
                out.push(format!("source {} needs a title and a url", s.id));
            }
        }
        let cite = |plant: &str, what: &str, source: &str, reference: &str, out: &mut Vec<String>| {
            if self.source(source).is_none() {
                out.push(format!("{plant}.{what}: unknown source {source:?}"));
            }
            if reference.trim().is_empty() {
                out.push(format!("{plant}.{what}: number without a ref"));
            }
        };
        for p in &self.plants {
            let id = p.id.as_str();
            if !ids.insert(format!("plant:{id}")) {
                out.push(format!("duplicate plant {id}"));
            }
            if id.is_empty() || !id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_') {
                out.push(format!("plant id {id:?} must be lowercase ascii"));
            }
            if p.scientific.trim().is_empty() || p.es.trim().is_empty() {
                out.push(format!("{id}: needs a scientific and a Spanish name"));
            }
            for r in &p.water {
                cite(id, "water", &r.source, &r.reference, &mut out);
                if r.days == 0 {
                    out.push(format!("{id}.water: 0 days"));
                }
            }
            for r in &p.fertilize {
                cite(id, "fertilize", &r.source, &r.reference, &mut out);
                if r.days == 0 {
                    out.push(format!("{id}.fertilize: 0 days"));
                }
            }
            if let Some(i) = &p.prune {
                cite(id, "prune", &i.source, &i.reference, &mut out);
            }
            if let Some(h) = &p.harvest {
                cite(id, "harvest", &h.source, &h.reference, &mut out);
                if h.to_days.is_some_and(|t| t < h.from_days) {
                    out.push(format!("{id}.harvest: to_days before from_days"));
                }
            }
            if let Some(m) = &p.mow {
                cite(id, "mow", &m.source, &m.reference, &mut out);
            }
            if !p.climates.is_empty() {
                match &p.climates_source {
                    Some(s) => cite(id, "climates", s, p.climates_ref.as_deref().unwrap_or(""), &mut out),
                    None => out.push(format!("{id}.climates: without climates_source")),
                }
            }
            let hint = |h: &Option<String>| h.as_deref().is_some_and(|s| !s.trim().is_empty());
            let topics: [(&str, bool, bool); 4] = [("water", !p.water.is_empty(), hint(&p.hints.water)), ("fertilize", !p.fertilize.is_empty(), hint(&p.hints.fertilize)), ("prune", p.prune.is_some(), hint(&p.hints.prune)), ("harvest", p.harvest.is_some(), hint(&p.hints.harvest))];
            for (what, number, has_hint) in topics {
                if !number && !has_hint {
                    out.push(format!("{id}.{what}: neither a sourced number nor a hint"));
                }
            }
            if p.kind == Kind::Prado && p.mow.is_none() && !hint(&p.hints.mow) {
                out.push(format!("{id}.mow: neither a sourced number nor a hint"));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(place: Option<Place>, climate: Option<Climate>, season: Option<Season>, days: u32) -> Rule {
        Rule { place, climate, season, days, product: None, source: "s".into(), reference: "p. 1".into() }
    }

    #[test]
    fn most_specific_rule_wins() {
        let rules = vec![rule(None, None, None, 7), rule(Some(Place::Matera), None, None, 3), rule(None, Some(Climate::Calido), None, 5), rule(Some(Place::Matera), Some(Climate::Calido), Some(Season::Seca), 2)];
        let d = |p, c, s| rule_for(&rules, p, c, s).map(|r| r.days);
        assert_eq!(d(Place::Huerta, None, Season::Seca), Some(7));
        assert_eq!(d(Place::Huerta, Some(Climate::Calido), Season::Seca), Some(5));
        assert_eq!(d(Place::Matera, Some(Climate::Frio), Season::Seca), Some(3), "place beats climate");
        assert_eq!(d(Place::Matera, Some(Climate::Calido), Season::Seca), Some(2));
        assert_eq!(d(Place::Matera, Some(Climate::Calido), Season::Lluviosa), Some(3));
        assert_eq!(rule_for(&rules[1..2], Place::Huerta, None, Season::Seca), None);
    }

    #[test]
    fn unsourced_numbers_and_missing_hints_are_problems() {
        let cat = r#"
schema = 1
[[plant]]
id = "x"
kind = "hortaliza"
scientific = "X x"
es = "Equis"
[[plant.water]]
days = 3
source = "nope"
[plant.harvest]
from_days = 60
source = "ok"
ref = "p. 2"
[plant.hints]
fertilize = "Abona con compost."
"#;
        let src = "schema = 1\n[[source]]\nid = \"ok\"\npublisher = \"P\"\ntitle = \"T\"\nurl = \"https://example.org\"\n";
        let c = Catalog::parse(cat, src).unwrap();
        let p = c.problems();
        assert!(p.contains(&"x.water: unknown source \"nope\"".to_string()), "{p:?}");
        assert!(p.contains(&"x.water: number without a ref".to_string()), "{p:?}");
        assert!(p.contains(&"x.prune: neither a sourced number nor a hint".to_string()), "{p:?}");
        assert!(!p.iter().any(|x| x.starts_with("x.harvest") || x.starts_with("x.fertilize")), "{p:?}");
        assert!(Catalog::parse("schema = 2", src).is_err());
    }
}
