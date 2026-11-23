# Sensors: Vigía and Nodo (design, v0.2)

Status: **design only**. v0.1 ships only the extension point in the data model
(`Garden::sensors`, `Garden::readings`, `model::{Sensor, SensorKind, Reading}`). Nothing in v0.1
reads or writes them, and the UI has no sensor screen. Everything below is optional: Mi Huerta keeps
working without any hardware.

## What could be connected

| Sensor | `SensorKind` | Unit | What Mi Huerta would do with it |
|---|---|---|---|
| Soil moisture probe (capacitive) | `soil_moisture` | % volumetric water content | Replace "every N days" watering with "when the soil is dry"; show the last reading on the plant |
| Temperature (air or soil) | `temperature` | °C | Frost or heat notes; later, adjust watering by temperature |
| Water tank level | `tank_level` | L or % | "Fill the tank" reminder; skip irrigation runs when it's empty |
| Irrigation valve or controller | `valve` | 1 = open, 0 = closed | Show and record what the controller did; confirm that a valve opened when a watering task is marked done |

A sensor belongs to a **plant** (`Sensor::plant`) or a **zone** (`Sensor::zone`, a free name such
as "bancal 1" or "tanque"), and may point to a device in Nodo (`Sensor::nodo_device`, an
`otherwise://nodo/device/<id>` id).

## Data flow

```
probe / controller ──(MQTT or HTTP on the LAN)──▶ Nodo ──▶ <data root>/nodo/…  (Nodo's own store)
                                                     │
                                                     └─▶ readings feed ──▶ Mi Huerta (read-only import)
                                                                         └─▶ Vigía (alerts)
```

1. **Nodo owns the devices.** Pairing, network, firmware, and the raw time series stay in Nodo,
   which already maps the user's devices. Nodo shows each sensor on its **device map** with the
   plant or zone name Mi Huerta gives it, plus a link back
   (`otherwise://mi-huerta/plant/<id>`).
2. **Mi Huerta reads, it doesn't talk to hardware.** A v0.2 job (`mi-huerta-cli background run
   sensors`, every 15 min, no network beyond the LAN-local Nodo files) imports the latest readings
   for the sensors it knows from Nodo's feed into `Garden::readings`. It keeps a short window, about
   30 days at hourly resolution, so `garden.json` stays small. Long history stays in Nodo.
3. **Scheduling.** For a plant with a soil-moisture sensor, the watering task becomes due when the
   last reading falls below the plant's threshold, instead of waiting for its interval. The
   interval stays as a fallback when readings stop. The threshold is user-editable. It is not in
   the catalogue until a source gives one per crop; the catalogue's rule ("every number cites a
   source") applies to thresholds too.
4. **Valves.** Mi Huerta never opens a valve by itself. Marking "Regar" done on a plant with a
   valve may *propose* an irrigation run to Nodo. The user confirms it, Nodo executes it, and the
   reading (`valve` = 1, then 0) confirms it.

## Vigía alerts

Vigía already watches things that must keep working. It would get three rule kinds through the
same feed. Each one is a Vigía "item" with a Today event and a notification:

| Alert | Condition (defaults user-editable) | Severity |
|---|---|---|
| Sensor stopped reporting | no reading for 6 h (soil, temperature) or 24 h (tank) | normal → high after 48 h |
| Soil too dry | soil moisture below the plant's threshold for 2 consecutive readings and no watering logged since | high |
| Valve failed to open | an irrigation run was sent but no `valve = 1` reading within 5 min, or the tank level didn't drop | high |

Vigía links each alert to the plant in Mi Huerta and to the device in Nodo.

## Formats (proposed; to be fixed in otherwise-link's SPEC before v0.2)

A sensor readings feed in `<data root>/nodo/readings/<device>.json`:

```json
{"schema": 1, "device": "esp32-patio", "sensors": [
  {"id": "soil-1", "kind": "soil_moisture", "unit": "%", "readings": [["2026-11-23T06:00:00-05:00", 31.5]]}
]}
```

Mi Huerta maps `Sensor::id` to the feed's `<device>/<sensor id>`. Readings in `garden.json` use
`model::Reading { sensor, at, value, unit }`.

## Privacy and safety

- Everything stays on the local network and in the data root. No cloud service is involved.
- Readings are not sent to the AI assistant unless the user asks about a plant. Even then only
  that plant's summary is sent (the last value and the time), never the raw series.
- Valve actions always need the user's confirmation, through a card or a button.

## Tests to add in v0.2

- Watering becomes due by threshold, and falls back to the interval when readings are stale.
- Import is idempotent and the window is trimmed.
- Vigía rules fire on synthetic feeds for each of the three alert kinds.
