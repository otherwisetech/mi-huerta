# mi-huerta — for agents and contributors

**Open source** (MIT OR Apache-2.0), public repo `otherwisetech/mi-huerta`. Never copy code from
closed Otherwise repos (expense-rs, voltia-rs, surco-rs…). Use otherwise-link,
otherwise-agent-panel, gpui-component and the open otherwise-apps as references. Read
`docs/SPEC.md` before touching `crates/core`.

## Layout

See README. `crates/core` (lib `huerta`) has **no UI and no GPUI**: keep it portable for a
future mobile app. `crates/cli` is headless. Only `crates/app` uses GPUI.

## Rules

- **Catalogue:** every number in `data/catalog.toml` cites `source` + `ref` (`Catalog::problems`,
  `crates/core/tests/catalog.rs`). Never invent or "estimate" a value. Where there is no source,
  leave a `hint` and let the user set the interval. Don't copy copyrighted tables.
- **Weather is opt-in.** Nothing goes on the network unless `settings.weather.enabled`. Only
  rounded coordinates (`weather::COORD_DECIMALS`) or the typed city name are sent, and the
  Settings tab shows the exact URL. Keep that disclosure accurate when changing requests.
- **AI writes are proposals.** The MCP server never writes `garden.json`. `propose_*` tools drop
  files into the app's inbox, and the app applies a proposal only when the user accepts its card.
- **Storage:** the garden folder defaults to `otherwise_link::paths::app_dir("mi-huerta")`.
  - The app holds the otherwise-link lock while the garden is open.
  - Saves are atomic and refuse a newer revision.
  - Save As copies and Move moves; both are verified and never overwrite.
  - Photos are copied in, never moved.
  - Unknown JSON fields are kept (`extra`).
- **Background jobs:** `reminders` only reads the garden. `forecast` writes it under the lock and
  stays idle while the app has it open. Both take the background lease.
- **i18n:** Spanish first, warm and plain. Every user-facing string goes through `i18n::t!` and
  exists in all four locales. Untranslatable literals end their line with `// i18n-ok`. The guard
  scans `crates/{app,core,cli,i18n}/src`.
- **Tests:** use synthetic data and temp dirs only, never the network or the real data root.
- **GPUI pins:** `Cargo.lock` pins zed `eb5483528be6…` and gpui-component `972a3ebfd01a…`. Never
  run a bare `cargo update`.
- **Never anything from Boardera.**

## Build (shared 16 GB Mac)

One build at a time across agents. Check `memory_pressure` and running rustc processes first.

```
export CARGO_TARGET_DIR=~/.cache/mi-huerta-target CARGO_BUILD_JOBS=4 CARGO_NET_GIT_FETCH_WITH_CLI=true
cargo test -p mi-huerta-i18n -p mi-huerta-core -p mi-huerta-cli -p mi-huerta-app
cargo clippy --workspace --all-targets -- -D warnings
script/bundle-macos.sh            # release .app in $CARGO_TARGET_DIR/release/
python3 script/make-icon.py       # regenerate icons (Pillow)
```

## Release

Bump `workspace.package.version`, then merge to main. Then either tag `vX.Y.Z` (the release
workflow builds macOS, Linux and Windows), or bundle locally and run
`gh release create vX.Y.Z <zip> <zip>.sha256`.

Notes, file names and issue forms follow the family's release rules: otherwisetech/otherwise-apps
`docs/releases.md` (how the Hub discovers and verifies releases).
