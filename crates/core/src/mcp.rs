//! A small stdio MCP server for the garden (JSON-RPC 2.0, one message per line: `initialize`,
//! `ping`, `tools/list`, `tools/call`). **Read-only**: the garden is opened without a lock and
//! nothing here writes it. With an inbox (`mi-huerta-cli mcp --propose --inbox <dir>`, what the
//! app's AI panel starts in "Preguntar antes de cambiar" and "Automático") it also offers
//! `propose_*` tools: each one is checked, then written as a proposal file into the inbox,
//! which the running app shows as an Aplicar / Rechazar card. Only the app applies it.
use crate::catalog::{Catalog, Species};
use crate::model::Garden;
use crate::ops::Op;
use crate::schedule::{self, Bucket};
use crate::store::Store;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

pub const PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// A proposal waiting in the inbox for the app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Proposal {
    pub id: String,
    pub op: Op,
    /// The agent's one-line reason, shown on the card.
    #[serde(default)]
    pub why: String,
    pub created_at: String,
}

/// Writes a proposal file into `inbox` (atomic; the app picks it up).
pub fn write_proposal(inbox: &Path, op: Op, why: &str) -> std::io::Result<Proposal> {
    // the app creates the inbox and removes it when it quits: no folder, nobody listening
    if !inbox.is_dir() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "no inbox"));
    }
    let p = Proposal { id: crate::model::new_id("prop"), op, why: why.chars().take(300).collect(), created_at: chrono::Utc::now().to_rfc3339() };
    let json = serde_json::to_vec_pretty(&p).map_err(std::io::Error::other)?;
    otherwise_link::fsutil::write_atomic(&inbox.join(format!("{}.json", p.id)), &json)?;
    Ok(p)
}

/// Takes every proposal out of `inbox` (oldest first); unreadable files are dropped.
pub fn take_proposals(inbox: &Path) -> Vec<Proposal> {
    let Ok(rd) = std::fs::read_dir(inbox) else { return vec![] };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || !name.ends_with(".json") {
            continue;
        }
        let p = std::fs::read_to_string(e.path()).ok().filter(|s| s.len() < 64 << 10).and_then(|s| serde_json::from_str::<Proposal>(&s).ok());
        if std::fs::remove_file(e.path()).is_ok()
            && let Some(p) = p
        {
            out.push(p);
        }
    }
    out.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    out
}

pub struct Server {
    pub dir: PathBuf,
    pub catalog: &'static Catalog,
    /// Where proposals go; `None`: read-only, no `propose_*` tools.
    pub inbox: Option<PathBuf>,
    pub today: fn() -> NaiveDate,
}

const HELP: &str = "Mi Huerta's home garden: plants (with their catalogue care rules and cited sources), care tasks (water, fertilize, prune, mow, harvest), journal, supplies, lawns and rain. Read-only. Catalogue numbers come with their source; when a value has no source the catalogue gives a hint instead: never invent intervals or doses, say what the source says or that there is none.";
const PROPOSE_HELP: &str = " The propose_* tools change nothing by themselves: each becomes a card in the Mi Huerta app that the user accepts or rejects (the answer says awaiting_approval). Tell the user to accept the card; never claim the change was made.";

struct Tool {
    /// The tool's MCP name (an identifier, not UI text).
    id: &'static str,
    description: &'static str,
    schema: fn() -> Value,
}

fn obj(props: Value, required: &[&str]) -> Value {
    json!({"type": "object", "properties": props, "required": required})
}

const READ_TOOLS: &[Tool] = &[
    Tool { id: "garden_summary", description: "The garden at a glance: climate, season, number of plants and lawns, tasks overdue / today / this week, supplies running low, weather status.", schema: || obj(json!({}), &[]) },
    Tool { id: "list_plants", description: "Every plant (id, name, species, place, planting date, notes, photos count, user intervals).", schema: || obj(json!({"include_archived": {"type": "boolean"}}), &[]) },
    Tool {
        id: "plant_care",
        description: "Care rules for a plant or a catalogue species: watering and fertilising rules by place / climate / season, pruning, harvest window, mowing, hints, and the sources each number cites.",
        schema: || obj(json!({"plant": {"type": "string", "description": "a plant id or name from list_plants"}, "species": {"type": "string", "description": "a catalogue id (tomate, limon…)"}}), &[]),
    },
    Tool { id: "list_species", description: "The built-in catalogue: id, Spanish and scientific names, kind, places, climates.", schema: || obj(json!({}), &[]) },
    Tool {
        id: "tasks",
        description: "Care tasks with due dates. `when`: overdue | today | week (overdue + today + next 7 days, default) | all. Also lists plants missing an interval (with the catalogue's hint).",
        schema: || obj(json!({"when": {"type": "string", "enum": ["overdue", "today", "week", "all"]}, "plant": {"type": "string"}}), &[]),
    },
    Tool { id: "journal", description: "Journal entries (newest first): date, plant, note, pests seen, harvest amount.", schema: || obj(json!({"plant": {"type": "string"}, "limit": {"type": "integer"}}), &[]) },
    Tool { id: "supplies", description: "Supplies (abono, tierra, semillas, insecticida…) with quantities, the running-low flag, and purchases with their cost.", schema: || obj(json!({}), &[]) },
    Tool { id: "weather", description: "Whether weather is on, the coarse location, the last forecast (rain mm per day) and the rain log.", schema: || obj(json!({}), &[]) },
];

const PROPOSE_TOOLS: &[Tool] = &[
    Tool {
        id: "propose_add_plant",
        description: "Propose adding a plant. place: matera | huerta | jardin | prado; species: a catalogue id from list_species (or omit); planted: YYYY-MM-DD.",
        schema: || obj(json!({"name": {"type": "string"}, "species": {"type": "string"}, "place": {"type": "string", "enum": ["matera", "huerta", "jardin", "prado"]}, "planted": {"type": "string"}, "notes": {"type": "string"}, "why": {"type": "string"}}), &["name", "place"]),
    },
    Tool {
        id: "propose_mark_task",
        description: "Propose marking a task done, skipped or snoozed (days). task: water | fertilize | prune | mow | harvest; target: the plant or lawn id.",
        schema: || obj(json!({"task": {"type": "string", "enum": ["water", "fertilize", "prune", "mow", "harvest"]}, "target": {"type": "string"}, "mark": {"type": "string", "enum": ["done", "skip", "snooze"]}, "days": {"type": "integer"}, "why": {"type": "string"}}), &["task", "target", "mark"]),
    },
    Tool {
        id: "propose_journal_entry",
        description: "Propose a journal entry: a note, pests seen, a harvest amount and unit.",
        schema: || obj(json!({"plant": {"type": "string"}, "note": {"type": "string"}, "pests": {"type": "string"}, "harvest_amount": {"type": "number"}, "harvest_unit": {"type": "string"}, "date": {"type": "string"}, "why": {"type": "string"}}), &["note"]),
    },
    Tool {
        id: "propose_add_supply",
        description: "Propose adding a supply. kind: abono | tierra | semillas | insecticida | herramienta | otro.",
        schema: || obj(json!({"name": {"type": "string"}, "kind": {"type": "string"}, "quantity": {"type": "number"}, "unit": {"type": "string"}, "low_at": {"type": "number"}, "why": {"type": "string"}}), &["name"]),
    },
    Tool { id: "propose_log_rain", description: "Propose logging rain (mm optional) on a date (default today).", schema: || obj(json!({"date": {"type": "string"}, "mm": {"type": "number"}, "why": {"type": "string"}}), &[]) },
    Tool {
        id: "propose_set_interval",
        description: "Propose the user's own interval in days for a plant's watering, fertilising, pruning or harvest (when the catalogue has no sourced value, or the user wants another one).",
        schema: || obj(json!({"target": {"type": "string"}, "task": {"type": "string", "enum": ["water", "fertilize", "prune", "harvest"]}, "days": {"type": "integer"}, "why": {"type": "string"}}), &["target", "task", "days"]),
    },
];

fn species_json(s: &Species, cat: &Catalog) -> Value {
    let sources: Vec<Value> = s.cited().iter().filter_map(|id| cat.source(id)).map(|src| json!({"id": src.id, "publisher": src.publisher, "title": src.title, "year": src.year, "url": src.url})).collect();
    json!({
        "id": s.id, "name": s.es, "scientific": s.scientific, "kind": s.kind.id(),
        "places": s.places, "climates": s.climates,
        "water": s.water, "fertilize": s.fertilize, "prune": s.prune, "harvest": s.harvest, "mow": s.mow,
        "hints": s.hints, "sources": sources,
    })
}

fn find_plant<'a>(g: &'a Garden, q: &str) -> Option<&'a crate::model::Plant> {
    let q = q.trim();
    g.plant(q).or_else(|| g.plants.iter().find(|p| otherwise_link::text::fold(&p.name) == otherwise_link::text::fold(q)))
}

impl Server {
    fn garden(&self) -> Result<Garden, String> {
        Store::open_read_only(&self.dir).map(|s| s.garden.clone()).map_err(|e| e.to_string())
    }

    fn call_read(&self, name: &str, args: &Value) -> Result<Value, String> {
        let g = self.garden()?;
        let cat = self.catalog;
        let today = (self.today)();
        let s = |k: &str| args.get(k).and_then(Value::as_str).map(str::to_string);
        Ok(match name {
            "garden_summary" => {
                let a = schedule::agenda(&g, cat, today);
                json!({
                    "today": today, "climate": g.settings.climate, "season": g.settings.season,
                    "plants": g.plants.iter().filter(|p| !p.archived).count(), "lawns": g.lawns.len(),
                    "overdue": a.in_bucket(Bucket::Overdue, today).len(), "due_today": a.in_bucket(Bucket::Today, today).len(), "this_week": a.this_week(today).len(),
                    "low_supplies": g.low_supplies().iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
                    "weather_on": g.settings.weather.enabled,
                })
            }
            "list_plants" => {
                let all = args.get("include_archived").and_then(Value::as_bool).unwrap_or(false);
                json!({"plants": g.plants.iter().filter(|p| all || !p.archived).map(|p| json!({"id": p.id, "name": p.name, "species": p.species, "place": p.place, "planted": p.planted, "covered": p.covered, "notes": p.notes, "photos": p.photos.len(), "overrides": p.overrides, "archived": p.archived})).collect::<Vec<_>>(), "lawns": g.lawns})
            }
            "plant_care" => {
                let (plant, species) = match (s("plant"), s("species")) {
                    (Some(q), _) => {
                        let p = find_plant(&g, &q).ok_or_else(|| format!("no plant {q:?}"))?;
                        (Some(p), p.species.clone())
                    }
                    (None, Some(sp)) => (None, sp),
                    _ => return Err("give plant or species".into()),
                };
                let sp = cat.get(&species);
                json!({"plant": plant.map(|p| json!({"id": p.id, "name": p.name, "place": p.place, "overrides": p.overrides})), "garden_climate": g.settings.climate, "season": g.settings.season, "species": sp.map(|s| species_json(s, cat)), "in_catalogue": sp.is_some()})
            }
            "list_species" => json!({"species": cat.plants.iter().map(|s| json!({"id": s.id, "name": s.es, "scientific": s.scientific, "kind": s.kind.id(), "places": s.places, "climates": s.climates})).collect::<Vec<_>>()}),
            "tasks" => {
                let a = schedule::agenda(&g, cat, today);
                let when = s("when").unwrap_or_else(|| "week".into());
                let plant = s("plant").and_then(|q| find_plant(&g, &q).map(|p| p.id.clone()));
                let tasks: Vec<_> = a
                    .tasks
                    .iter()
                    .filter(|t| match when.as_str() {
                        "overdue" => t.bucket(today) == Bucket::Overdue,
                        "today" => t.bucket(today) == Bucket::Today,
                        "all" => true,
                        _ => t.bucket(today) != Bucket::Later,
                    })
                    .filter(|t| plant.as_ref().is_none_or(|p| &t.target == p))
                    .map(|t| json!({"task": t.kind, "target": t.target, "name": t.name, "due": t.due, "until": t.until, "every_days": t.interval, "origin": t.origin, "rain": t.rain, "snoozed": t.snoozed, "bucket": t.bucket(today)}))
                    .collect();
                json!({"today": today, "tasks": tasks, "missing_interval": a.missing})
            }
            "journal" => {
                let plant = s("plant").and_then(|q| find_plant(&g, &q).map(|p| p.id.clone()));
                let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(30).min(200) as usize;
                let mut v: Vec<_> = g.journal.iter().filter(|j| plant.is_none() || j.plant == plant).collect();
                v.sort_by_key(|e| std::cmp::Reverse(e.date));
                json!({"entries": v.iter().take(limit).map(|j| json!({"id": j.id, "date": j.date, "plant": j.plant.as_deref().and_then(|p| g.target_name(p)), "note": j.note, "pests": j.pests, "harvest": j.harvest, "photos": j.photos.len()})).collect::<Vec<_>>()})
            }
            "supplies" => json!({"supplies": g.supplies.iter().map(|s| json!({"id": s.id, "name": s.name, "kind": s.kind, "quantity": s.quantity, "unit": s.unit, "low_at": s.low_at, "low": s.is_low()})).collect::<Vec<_>>(), "purchases": g.purchases, "spent": g.spent()}),
            "weather" => json!({"enabled": g.settings.weather.enabled, "place": g.settings.weather.place, "lat": g.settings.weather.lat, "lon": g.settings.weather.lon, "skip_mm": g.settings.weather.skip_mm, "forecast": g.forecast, "rain": g.rain.iter().rev().take(30).collect::<Vec<_>>()}),
            _ => return Err(format!("unknown tool {name:?}")),
        })
    }

    fn call_propose(&self, inbox: &Path, name: &str, args: &Value) -> Result<Value, String> {
        let mut a = args.clone();
        let why = a.get("why").and_then(Value::as_str).unwrap_or_default().to_string();
        if let Some(o) = a.as_object_mut() {
            o.remove("why");
            let op = match name {
                "propose_add_plant" => "add_plant",
                "propose_mark_task" => "mark_task",
                "propose_journal_entry" => "add_journal",
                "propose_add_supply" => "add_supply",
                "propose_log_rain" => "log_rain",
                "propose_set_interval" => "set_interval",
                _ => return Err(format!("unknown tool {name:?}")),
            };
            o.insert("op".into(), json!(op));
        }
        let op: Op = serde_json::from_value(a).map_err(|e| e.to_string())?;
        let g = self.garden()?;
        op.check(&g, self.catalog)?;
        let p = write_proposal(inbox, op, &why).map_err(|e| format!("the Mi Huerta app is not listening for proposals ({e}); describe the change to the user instead"))?;
        Ok(json!({"pending": p.id, "status": "awaiting_approval", "message": "Shown to the user as a card in Mi Huerta; it is applied only if they accept it."}))
    }

    pub fn handle(&self, msg: &Value) -> Option<Value> {
        let id = msg.get("id").cloned()?;
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let err = |code: i64, m: &str| json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": m}});
        let ok = |r: Value| json!({"jsonrpc": "2.0", "id": id, "result": r});
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        Some(match method {
            "initialize" => {
                let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
                let v = PROTOCOL_VERSIONS.iter().find(|v| **v == asked).copied().unwrap_or(PROTOCOL_VERSIONS[0]);
                let help = if self.inbox.is_some() { format!("{HELP}{PROPOSE_HELP}") } else { HELP.to_string() };
                ok(json!({"protocolVersion": v, "capabilities": {"tools": {"listChanged": false}}, "serverInfo": {"name": crate::store::APP, "version": env!("CARGO_PKG_VERSION")}, "instructions": help}))
            }
            "ping" => ok(json!({})),
            "tools/list" => {
                let read = READ_TOOLS.iter().map(|t| json!({"name": t.id, "description": t.description, "inputSchema": (t.schema)(), "annotations": {"readOnlyHint": true}}));
                let prop = PROPOSE_TOOLS.iter().filter(|_| self.inbox.is_some()).map(|t| json!({"name": t.id, "description": format!("{} Proposes it only: the user accepts or rejects it in Mi Huerta.", t.description), "inputSchema": (t.schema)(), "annotations": {"readOnlyHint": false, "destructiveHint": false}}));
                ok(json!({"tools": read.chain(prop).collect::<Vec<_>>()}))
            }
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                let res = if READ_TOOLS.iter().any(|t| t.id == name) {
                    self.call_read(name, &args)
                } else if let Some(inbox) = self.inbox.as_ref().filter(|_| PROPOSE_TOOLS.iter().any(|t| t.id == name)) {
                    self.call_propose(inbox, name, &args)
                } else {
                    return Some(err(-32602, &format!("unknown tool {name:?}")));
                };
                match res {
                    Ok(v) => ok(json!({"content": [{"type": "text", "text": v.to_string()}], "structuredContent": v, "isError": false})),
                    Err(e) => ok(json!({"content": [{"type": "text", "text": e}], "isError": true})),
                }
            }
            _ => err(-32601, &format!("method not found: {method}")),
        })
    }

    pub fn serve(&self, input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
        for line in input.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let reply = match serde_json::from_str::<Value>(&line) {
                Ok(Value::Array(batch)) => {
                    let r: Vec<Value> = batch.iter().filter_map(|m| self.handle(m)).collect();
                    (!r.is_empty()).then_some(Value::Array(r))
                }
                Ok(m) => self.handle(&m),
                Err(_) => Some(json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "parse error"}})),
            };
            if let Some(r) = reply {
                writeln!(output, "{r}")?;
                output.flush()?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Plant;

    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 23).unwrap()
    }

    fn setup(name: &str) -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("mi-huerta-mcp-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let dir = base.join("garden");
        let mut s = Store::open(&dir).unwrap();
        s.garden.plants.push(Plant { id: "p1".into(), name: "Tomatera".into(), species: "tomate".into(), added: Some(day()), ..Default::default() });
        s.save().unwrap();
        drop(s);
        let inbox = base.join("inbox");
        std::fs::create_dir_all(&inbox).unwrap();
        (dir, inbox)
    }

    fn call(srv: &Server, name: &str, args: Value) -> Value {
        srv.handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": name, "arguments": args}})).unwrap()["result"].clone()
    }

    #[test]
    fn read_only_by_default_and_proposals_go_to_the_inbox() {
        let (dir, inbox) = setup("ro");
        let mut srv = Server { dir: dir.clone(), catalog: Catalog::builtin(), inbox: None, today: day };
        let list = srv.handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})).unwrap();
        let tools = list["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), READ_TOOLS.len());
        assert!(tools.iter().all(|t| t["annotations"]["readOnlyHint"] == true));
        let care = call(&srv, "plant_care", json!({"plant": "tomatera"}));
        assert_eq!(care["structuredContent"]["in_catalogue"], true);
        assert_eq!(call(&srv, "tasks", json!({"when": "all"}))["isError"], false);
        let refused = srv.handle(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "propose_log_rain", "arguments": {}}})).unwrap();
        assert_eq!(refused["error"]["code"], -32602);

        srv.inbox = Some(inbox.clone());
        let before = std::fs::read_to_string(dir.join(crate::store::FILE)).unwrap();
        let r = call(&srv, "propose_mark_task", json!({"task": "water", "target": "p1", "mark": "done", "why": "Lo regaste hoy"}));
        assert_eq!(r["structuredContent"]["status"], "awaiting_approval");
        let bad = call(&srv, "propose_mark_task", json!({"task": "water", "target": "nope", "mark": "done"}));
        assert_eq!(bad["isError"], true, "checked before it reaches the app");
        assert_eq!(std::fs::read_to_string(dir.join(crate::store::FILE)).unwrap(), before, "nothing written to the garden");
        assert!(!dir.join("garden.json.lock.json").exists(), "no lock taken");
        let props = take_proposals(&inbox);
        assert_eq!(props.len(), 1);
        assert_eq!(props[0].why, "Lo regaste hoy");
        assert!(matches!(props[0].op, Op::MarkTask { .. }));
        assert!(take_proposals(&inbox).is_empty(), "taken once");
        // a vanished inbox: the agent is told
        std::fs::remove_dir_all(&inbox).unwrap();
        let r = call(&srv, "propose_log_rain", json!({"mm": 3.0}));
        assert_eq!(r["isError"], true);
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}
