//! The text fields of every tab, created once with the window.
use gpui::*;
use gpui_component::input::InputState;

pub struct Forms {
    pub plant_name: Entity<InputState>,
    pub plant_planted: Entity<InputState>,
    pub plant_notes: Entity<InputState>,
    pub edit_notes: Entity<InputState>,
    pub water_days: Entity<InputState>,
    pub fertilize_days: Entity<InputState>,
    pub prune_days: Entity<InputState>,
    pub harvest_days: Entity<InputState>,
    pub journal_note: Entity<InputState>,
    pub journal_pests: Entity<InputState>,
    pub journal_amount: Entity<InputState>,
    pub journal_unit: Entity<InputState>,
    pub supply_name: Entity<InputState>,
    pub supply_qty: Entity<InputState>,
    pub supply_unit: Entity<InputState>,
    pub supply_low: Entity<InputState>,
    pub buy_qty: Entity<InputState>,
    pub buy_cost: Entity<InputState>,
    pub buy_shop: Entity<InputState>,
    pub lawn_name: Entity<InputState>,
    pub lawn_days: Entity<InputState>,
    pub lawn_height: Entity<InputState>,
    pub rain_mm: Entity<InputState>,
    pub city: Entity<InputState>,
    pub skip_mm: Entity<InputState>,
}

fn field(window: &mut Window, cx: &mut App, placeholder: String) -> Entity<InputState> {
    cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
}

impl Forms {
    pub fn new(window: &mut Window, cx: &mut App) -> Forms {
        let t = |k: &str| i18n::t!(k);
        Forms {
            plant_name: field(window, cx, t("app.plants.name_placeholder")),
            plant_planted: field(window, cx, t("app.date_placeholder")),
            plant_notes: field(window, cx, t("app.plants.notes_placeholder")),
            edit_notes: field(window, cx, t("app.plants.notes_placeholder")),
            water_days: field(window, cx, t("app.days_placeholder")),
            fertilize_days: field(window, cx, t("app.days_placeholder")),
            prune_days: field(window, cx, t("app.days_placeholder")),
            harvest_days: field(window, cx, t("app.days_placeholder")),
            journal_note: field(window, cx, t("app.journal.note_placeholder")),
            journal_pests: field(window, cx, t("app.journal.pests_placeholder")),
            journal_amount: field(window, cx, t("app.journal.amount_placeholder")),
            journal_unit: field(window, cx, t("app.journal.unit_placeholder")),
            supply_name: field(window, cx, t("app.supplies.name_placeholder")),
            supply_qty: field(window, cx, t("app.supplies.qty_placeholder")),
            supply_unit: field(window, cx, t("app.supplies.unit_placeholder")),
            supply_low: field(window, cx, t("app.supplies.low_placeholder")),
            buy_qty: field(window, cx, t("app.supplies.qty_placeholder")),
            buy_cost: field(window, cx, t("app.supplies.cost_placeholder")),
            buy_shop: field(window, cx, t("app.supplies.shop_placeholder")),
            lawn_name: field(window, cx, t("app.lawn.name_placeholder")),
            lawn_days: field(window, cx, t("app.days_placeholder")),
            lawn_height: field(window, cx, t("app.lawn.height_placeholder")),
            rain_mm: field(window, cx, t("app.today.rain_placeholder")),
            city: field(window, cx, t("app.settings.city_placeholder")),
            skip_mm: field(window, cx, t("app.settings.skip_placeholder")),
        }
    }
}

pub fn text(st: &Entity<InputState>, cx: &App) -> String {
    st.read(cx).value().trim().to_string()
}

/// A number typed with a comma or a dot.
pub fn number(st: &Entity<InputState>, cx: &App) -> Option<f64> {
    text(st, cx).replace(',', ".").parse::<f64>().ok().filter(|v| v.is_finite())
}

pub fn days(st: &Entity<InputState>, cx: &App) -> Option<u32> {
    text(st, cx).parse::<u32>().ok().filter(|d| *d > 0 && *d <= 730)
}

/// `YYYY-MM-DD`, or `DD/MM/YYYY` as Colombians write it.
pub fn date(st: &Entity<InputState>, cx: &App) -> Option<chrono::NaiveDate> {
    let s = text(st, cx);
    chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").or_else(|_| chrono::NaiveDate::parse_from_str(&s, "%d/%m/%Y")).ok()
}

pub fn clear(st: &Entity<InputState>, window: &mut Window, cx: &mut App) {
    st.update(cx, |s, cx| s.set_value("", window, cx));
}

pub fn put(st: &Entity<InputState>, v: String, window: &mut Window, cx: &mut App) {
    st.update(cx, |s, cx| s.set_value(v, window, cx));
}
