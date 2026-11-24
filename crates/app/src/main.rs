// No console window behind the app on Windows release builds.
#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

mod agent;
mod app;
mod forms;
mod theme;
mod ui;
mod views;

use gpui::*;
use gpui_component::Root;
use gpui_component_assets::Assets;
use otherwise_link::Link;
use std::borrow::Cow;
use std::sync::Mutex;

actions!(mi_huerta, [Quit, OpenWebsite, OpenVcity, OpenSource]);

pub(crate) const APP_NAME: &str = "Mi Huerta"; // i18n-ok: product name
pub(crate) const COMPANY: &str = otherwise_ui::COMPANY;
pub(crate) const COMPANY_URL: &str = otherwise_ui::COMPANY_URL;
/// The invitation to Vcity (About, Help): campaign tags only, nothing about the user.
pub(crate) const VCITY_URL: &str = "https://vcity.io/?utm_source=mi-huerta&utm_medium=app&utm_campaign=otherwise-apps";
pub(crate) const REPO_URL: &str = "https://github.com/otherwisetech/mi-huerta";
pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");
pub(crate) const LOGO_SVG: &str = "icons/mi-huerta-mark.svg";
pub(crate) static ICON_PNG: &[u8] = include_bytes!("../assets/logo/mi-huerta-icon-512.png");

/// The native menu bar holds plain strings, so it is rebuilt on every language switch.
pub(crate) fn set_app_menus(cx: &mut App) {
    let items = vec![
        MenuItem::action(i18n::t!("app.about.menu", app = APP_NAME), app::ShowAbout),
        MenuItem::separator(),
        MenuItem::action(i18n::t!("app.credit.menu", company = COMPANY), OpenWebsite),
        MenuItem::separator(),
        MenuItem::action(i18n::t!("app.menu.quit"), Quit),
    ];
    let garden = vec![
        MenuItem::action(i18n::t!("app.tab.today"), app::ShowToday),
        MenuItem::action(i18n::t!("app.tab.plants"), app::ShowPlants),
        MenuItem::action(i18n::t!("app.tab.journal"), app::ShowJournal),
        MenuItem::action(i18n::t!("app.tab.supplies"), app::ShowSupplies),
        MenuItem::action(i18n::t!("app.tab.lawn"), app::ShowLawn),
        MenuItem::separator(),
        MenuItem::action(i18n::t!("app.file.save_as"), app::SaveAs),
        MenuItem::action(i18n::t!("app.file.move"), app::MoveGarden),
        MenuItem::action(i18n::t!("app.file.open"), app::OpenGarden),
        MenuItem::action(app::reveal_label(), app::Reveal),
    ];
    let help = vec![MenuItem::action(i18n::t!("app.vcity.menu"), OpenVcity), MenuItem::action(i18n::t!("app.about.source"), OpenSource), MenuItem::separator(), MenuItem::action(i18n::t!("app.credit.menu", company = COMPANY), OpenWebsite)];
    cx.set_menus(vec![
        Menu { name: APP_NAME.into(), items, disabled: false },
        Menu { name: i18n::t!("app.menu.garden").into(), items: garden, disabled: false },
        Menu { name: i18n::t!("app.menu.help").into(), items: help, disabled: false },
    ]);
}

/// gpui-component's icon set plus our mark.
struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path == LOGO_SVG {
            return Ok(Some(Cow::Borrowed(include_bytes!("../assets/logo/mi-huerta-mark.svg"))));
        }
        Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Assets.list(path)
    }
}

/// Sets the Dock icon at runtime: covers `cargo run` (no bundle) and stale Dock icon caches.
#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)] // objc 0.2 macros check a `cargo-clippy` feature
fn set_dock_icon() {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    static PNG: &[u8] = include_bytes!("../assets/logo/mi-huerta-icon-1024.png");
    // SAFETY: plain AppKit calls on the main thread with a valid byte buffer that NSData copies.
    unsafe {
        let data: *mut Object = msg_send![class!(NSData), dataWithBytes: PNG.as_ptr() length: PNG.len()];
        let image: *mut Object = msg_send![class!(NSImage), alloc];
        let image: *mut Object = msg_send![image, initWithData: data];
        if image.is_null() {
            return;
        }
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, setApplicationIconImage: image];
        let _: () = msg_send![image, release];
    }
}

/// `otherwise-mi-huerta://` links waiting for the window.
static INBOUND: Mutex<Vec<Link>> = Mutex::new(Vec::new());

fn push_inbound(links: impl IntoIterator<Item = Link>) {
    INBOUND.lock().unwrap_or_else(|e| e.into_inner()).extend(links);
}

pub(crate) fn take_inbound() -> Vec<Link> {
    std::mem::take(&mut *INBOUND.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Windows and Linux learn about `otherwise-mi-huerta://` at run time (macOS from Info.plist).
fn register_scheme() {
    let Ok(exe) = std::env::current_exe() else { return };
    std::thread::spawn(move || {
        let scheme = otherwise_link::link::app_scheme(huerta::store::APP);
        if cfg!(windows) {
            let _ = otherwise_link::register::register_windows(&scheme, &exe, APP_NAME);
        } else if cfg!(target_os = "linux") {
            let file = "com.otherwisetech.mi-huerta.desktop"; // i18n-ok: file name
            if otherwise_link::paths::home().join(".local/share/applications").join(file).is_file() {
                let _ = otherwise_link::register::register_linux(file, &[scheme.as_str()]);
            }
        }
    });
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // `mi-huerta mcp | background | calendar | tasks`: the command line, no window
    if let Some(code) = mi_huerta_cli::run(&args) {
        std::process::exit(code);
    }
    if matches!(args.first().map(String::as_str), Some("-h" | "--help")) {
        println!("{}", mi_huerta_cli::usage());
        return;
    }
    let local = huerta::local::Local::load();
    let lang = local.language.as_deref().and_then(i18n::match_system).or_else(|| std::env::var("LANG").ok().as_deref().and_then(i18n::match_system)).unwrap_or(i18n::DEFAULT);
    i18n::set_locale(lang);
    push_inbound(Link::from_args(&args));
    register_scheme();
    let application = gpui_platform::application().with_assets(AppAssets);
    application.on_open_urls(|urls| push_inbound(urls.iter().filter_map(|u| Link::parse(u).ok())));
    application.run(move |cx| {
        #[cfg(target_os = "macos")]
        set_dock_icon();
        gpui_component::init(cx);
        otherwise_ui::init(otherwise_ui::accents::MI_HUERTA, cx);
        theme::apply(local.theme.as_deref(), None, cx);
        app::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("secondary-q", Quit, None)]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_action(|_: &OpenWebsite, cx| cx.open_url(COMPANY_URL));
        cx.on_action(|_: &OpenVcity, cx| cx.open_url(VCITY_URL));
        cx.on_action(|_: &OpenSource, cx| cx.open_url(REPO_URL));
        gpui_component::set_locale(i18n::locale());
        otherwise_agent_panel::ui::init(cx);
        agent::set_panel_locale(i18n::locale());
        set_app_menus(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(1240.), px(820.)), cx)),
            titlebar: Some(TitlebarOptions { title: Some(APP_NAME.into()), ..Default::default() }),
            window_min_size: Some(size(px(760.), px(520.))),
            ..Default::default()
        };
        cx.spawn(async move |cx| {
            cx.open_window(window_options, |window, cx| {
                let view = cx.new(|cx| app::HuertaApp::new(window, cx));
                let focus = view.read(cx).focus.clone();
                let host: std::rc::Rc<dyn otherwise_agent_panel::ui::AgentHost> = std::rc::Rc::new(agent::Host { app: view.downgrade() });
                let dock = cx.new(|cx| otherwise_agent_panel::ui::AgentDock::new(view.clone(), Some(focus), host, window, cx));
                let panel = dock.read(cx).panel().downgrade();
                view.update(cx, |v, _| v.panel = Some(panel));
                cx.new(|cx| Root::new(dock, window, cx))
            })
            .expect("open window");
        })
        .detach();
    });
}
