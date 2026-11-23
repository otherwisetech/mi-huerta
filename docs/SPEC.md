# Mi Huerta — specification (v0.1)

A free, open-source (MIT OR Apache-2.0) desktop app by Otherwise Technologies LLC to look after a
home garden: tomato plants, lemongrass, herbs, fruit trees, potted plants and the lawn. It is the
free personal sibling of Surco, the paid agro suite. Desktop first: `crates/core` has no UI so a
mobile app can reuse it later.

## 1. Catalogue (`data/catalog.toml`, `data/sources.toml`)

- 44 common Colombian home plants, each with its Colombian name, scientific name, kind, the
  places it is grown at home (matera, huerta, jardín, prado) and, when a source says so, the
  thermal floors it does well in.
- Care rules:
  - **Watering and fertilising:** a list of rules (days between applications). Each rule may be
    limited to a place, a climate (cálido / templado / frío) and a season (seca / lluviosa). The
    most specific rule wins: place counts more than climate, and climate more than season.
  - **Pruning:** an interval in days.
  - **Harvest:** days from sowing or transplanting, as a window.
  - **Mowing:** interval and height range.
- **Every number cites a source**: a `source` id from `sources.toml` plus a `ref` (page or
  section). Where no source was found, the number is absent and a plain-Spanish `hint` says how to
  decide. The user then sets their own interval. Unit conversions (months → 30 days, inches → cm)
  are listed at the top of the file. `Catalog::problems` enforces all of this, and
  `crates/core/tests/catalog.rs` fails on any problem.
- Sources are cited and paraphrased (single values). No table is copied.

## 2. Garden model (`crates/core/src/model.rs`)

`garden.json` (schema 1) holds:

- settings: climate, season, weather (off by default), calendar and notification switches;
- plants: name, catalogue species or a custom plant, place, planting date, the date it was added,
  whether it is under a roof, photos, notes, the user's own intervals, archived;
- lawns: mowing interval, height and last mowing;
- supplies (with a "running low" level) and purchases (quantity, cost, shop);
- journal entries: note, pests seen, harvest amount, photos;
- the care log (done / skipped / snoozed);
- rain entries (manual or Open-Meteo) and the last forecast;
- the **sensor extension point** (`sensors`, `readings`). It is unused in v0.1; see
  [sensors.md](sensors.md).

Unknown fields are kept and written back. A save refuses to overwrite a newer `revision`.

## 3. Scheduling (`crates/core/src/schedule.rs`)

- **Interval:** the user's override, else the catalogue rule for the plant's place, the garden's
  climate and the season. With neither, there is no task, and the plant is listed under
  "Falta un intervalo" with the catalogue's hint.
- **Due day:** the last done-or-skipped day, else the day the plant was added, else the planting
  day, else today; plus the interval.
- **Done / Saltar:** both reset the count. **Mañana:** snoozes to tomorrow; a snooze recorded after
  the last done or skip moves the due day.
- **Rain** (watering only, and not for plants under a roof):
  - Rain since the last watering counts as a watering when it is at least the threshold (5 mm by
    default, adjustable) or was logged without a measure. Manual rain always counts. Open-Meteo
    rain counts only while weather is on.
  - When weather is on, rain forecast on the due day or the next one postpones the task to that
    day.
- **Harvest:** once per planting, from planting + `from_days`, shown until `to_days`.
- **Lawns:** the lawn's own interval, else the catalogue's prado entry; counted from the last
  mowing.
- **Views:** overdue, today, this week (7 days).

## 4. Weather (`crates/core/src/weather.rs`)

Opt-in. Only `api.open-meteo.com/v1/forecast` is called, with latitude and longitude rounded to
one decimal (about 11 km), daily `precipitation_sum` and `precipitation_probability_max`, the past
day and the next 3. The city search sends the typed name to `geocoding-api.open-meteo.com`. The
Settings tab shows the exact URL that is sent. It is checked when the app opens (if the last check
is older than 6 h) and every 6 h by Otherwise Helper.

## 5. Storage (`crates/core/src/store.rs`)

- **Folder:** `<otherwise-link data root>/mi-huerta/` by default (`~/.otherwise/mi-huerta`). It
  holds `garden.json`, `photos/`, `events.json` (the calendar feed), `outbox/` (notifications) and
  `background.lock.json` (the Helper's lease).
- **Locking:** the app holds `garden.json.lock.json` (otherwise-link `Lock`, with a heartbeat).
  Open on another computer means read-only, with a **Usar aquí** button. Conflicted copies from
  sync clients are reported.
- **Save As / Move:** Save As makes a verified copy and switches to it. Move does a verified copy
  and then deletes the original. Neither ever overwrites anything.
- **Photos:** copied into `photos/`, never moved.
- **Local settings:** language, theme and a chosen folder live in the platform config folder.

## 6. AI (`crates/core/src/mcp.rs`, `crates/app/src/agent.rs`)

- The shared otherwise-agent-panel, toggled with ⌘J. It runs the user's own Claude Code, Codex or
  opencode with `mi-huerta-cli mcp --garden <dir>` attached for that session only.
- **Read tools** (`readOnlyHint`): `garden_summary`, `list_plants`, `plant_care` (with sources),
  `list_species`, `tasks`, `journal`, `supplies`, `weather`.
- **Proposals:** outside "Solo lectura", `--propose --inbox <dir>` adds `propose_add_plant`,
  `propose_mark_task`, `propose_journal_entry`, `propose_add_supply`, `propose_log_rain` and
  `propose_set_interval`.
  - Each proposal is checked, then written to a per-user temp inbox that the app polls every
    second.
  - The app shows it as an Aplicar / Rechazar card. Only an accepted card changes the garden.
  - "Automático" also proposes.
- **Starter prompts** (es / en / pt / fr): "¿Qué le pasa a mi tomatera?" (with a photo), "¿Qué toca
  esta semana?", "¿Cuándo abono el limonero?".
- **Dropped photos** can be sent with a question, or saved to the selected plant.
- **Privacy:** the panel's footer is overridden to say what of the garden goes to which provider.

## 7. Otherwise integration

- **Discovery:** `mi-huerta` is in otherwise-link `apps::KNOWN` (v0.5.1), with bundle id
  `com.otherwisetech.mi-huerta`, CLI `mi-huerta-cli`, MCP and Calendar. Otherwise Apps lists it, and
  the hub proxies its MCP.
- **Calendar:** `<data root>/mi-huerta/events.json` holds each task's next day, repeating with
  `FREQ=DAILY;INTERVAL=n`, and harvest windows as ranges. Links point to
  `otherwise://mi-huerta/plant/<id>`. The feed is cleared when the user turns it off.
- **Otherwise Helper:** `Contents/Resources/otherwise-background.json` is written at bundle time by
  `mi-huerta-cli background manifest`.
  - `reminders` runs daily at 07:00 without network. It sends "Hoy toca regar / abonar / cortar el
    prado: …" and "Se te está acabando: …" to the outbox, and refreshes the calendar.
  - `forecast` runs every 6 h with network, only when weather is on. It stays idle while the app
    has the garden open.
  - Both take the background lease, so only one computer runs them when the data root is synced.
- **Deep links:** `otherwise-mi-huerta://` (Info.plist, Windows and Linux registration), with the
  paths `plant/<id>`, `lawn`, `supplies`, `journal` and `today`.

## 8. Conventions

- About: logo, version, description, the credit "Otherwise Technologies LLC, Wyoming, USA" with a
  link to otherwisetech.com, the source code link, the MIT / Apache licence and the Vcity
  invitation.
- Help menu: "Crea tu tienda en Vcity" (`utm_source=mi-huerta`), source code, credit.
- Surco: the text "¿Tienes una finca? Conoce Surco" in the footer, About and Settings, with no link
  until Surco has a store.
- i18n: Spanish first, then en / pt / fr. The guard test (`crates/i18n/tests/guard.rs`) fails on
  hard-coded UI text, keys missing in a language and unused keys.
