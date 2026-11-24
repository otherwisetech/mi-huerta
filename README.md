# Mi Huerta

**Cuida tu huerta en casa.** A free, open-source desktop app for home gardens: tomato plants,
lemongrass, herbs, fruit trees, pots and the lawn. Mi Huerta tells you what to water, fertilise,
prune, mow and harvest today, using care rules whose every number cites a source.

Made by [Otherwise Technologies LLC](https://otherwisetech.com) (Wyoming, USA). Spanish first,
also in English, Portuguese and French. ¿Tienes una finca? Conoce Surco.

> Screenshots: _coming soon_ (`docs/screenshots/`).

## What it does

- **Mis plantas.** 44 common Colombian home plants (tomate, limoncillo, cilantro, albahaca, papa,
  plátano, lulo…) with their scientific names. For each plant you record the place (matera, huerta,
  jardín, prado), the planting date, photos (copied into your garden folder) and notes.
- **Calendario de cuidados.** Today, overdue and this week. Mark a task done in one click,
  postpone it to tomorrow, or skip it.
- **Every number has a source.** Values come from FAO, UF/IFAS, ICA, Corpoica/Agrosavia and UC
  IPM. Where no source exists, the catalogue says so, gives a hint, and you set your own interval.
- **Riego inteligente** (off by default). Rain you log, or rain from Open-Meteo, counts as
  watering, and forecast rain postpones it. When it's on, only a rounded location (about 11 km) is
  sent to Open-Meteo, and the app shows the exact request.
- **Insumos**, **Diario** (notes, pests, harvests, photos) and **Prado** (mowing interval and
  height).
- **AI assistant (⌘J).** Your own Claude Code, Codex or opencode, or a model of your own Ollama
  on this computer (everything stays local), with read-only access to your garden. Any change it suggests arrives as a card that you accept or reject.
- **Otherwise Apps.**
  - Care tasks appear in its calendar.
  - Daily "Hoy toca regar…" notifications arrive through Otherwise Helper, even when the app is
    closed.
  - The hub lists the app and its MCP tools.
- **Your data stays yours.** It lives in a folder under the Otherwise data root (`~/.otherwise` by
  default). You can move it to OneDrive or Dropbox safely: Mi Huerta locks the garden while it is
  open and reports conflicted copies.

## Build

Rust stable (see `rust-toolchain.toml`). The GPUI revisions are pinned in `Cargo.lock`; never run
a bare `cargo update`.

```sh
cargo test                                   # core, CLI, translations (no GPUI)
cargo test -p mi-huerta-app                  # the app's own tests
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p mi-huerta-app                   # the desktop app
script/bundle-macos.sh                       # → target/release/Mi Huerta.app
cargo run -p mi-huerta-cli -- tasks          # what is due this week, in the terminal
```

On Linux, install the libraries listed in `.github/workflows/release.yml`.

The command line (`mi-huerta-cli`, also answered by the app binary) provides:

- `tasks [--json]`;
- `calendar`;
- `mcp [--garden DIR] [--propose --inbox DIR]`;
- `background manifest | run reminders | run forecast`.

Deep links: `otherwise-mi-huerta://plant/<id>`, `…/today`, `…/supplies`, `…/journal`, `…/lawn`.

## Layout

```
data/            catalog.toml (care rules, each number with source + ref), sources.toml
crates/core      mi-huerta-core (lib `huerta`), no UI: catalog, model, schedule, weather, store,
                 local settings, ops, mcp, feeds (calendar + reminders)
crates/cli       mi-huerta-cli: MCP server, Otherwise Helper jobs, calendar feed
crates/i18n      translations (es, en, pt, fr) + the i18n guard tests
crates/app       the GPUI desktop app (gpui-component, otherwise-agent-panel)
docs/            SPEC.md, sensors.md (Vigía / Nodo design for v0.2)
script/          bundle-macos.sh, make-icon.py
```

## Contributing

Contributions are welcome, especially catalogue data with good sources.

- **Catalogue:** every number in `data/catalog.toml` needs a `source` (an id in
  `data/sources.toml` with a URL you actually read) and a `ref` (page or section). Paraphrase
  single values; don't copy tables. If you can't find a source, leave the number out and write a
  short, plain-Spanish `hint`. `cargo test` checks all of this.
- **Translations:** every UI string goes through `i18n::t!` and must exist in all four
  `crates/i18n/locales/<lang>/` catalogs. The guard test fails otherwise.
- **Tests:** every core change ships with a test. Use synthetic data only: no real gardens and no
  network.
- **Pull requests:** open them against `main`. CI runs the tests on macOS, Linux and Windows, plus
  clippy.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
Catalogue values are cited from their publishers (see `data/sources.toml`). The Inter font
(SIL Open Font License) comes with otherwise-ui.
