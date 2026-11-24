//! The family's tokens come from otherwise-ui (github.com/otherwisetech/otherwise-ui, by tag);
//! Mi Huerta only picks its leaf-green accent (`otherwise_ui::accents::MI_HUERTA`, set in
//! main.rs). The rain colour is the family's `info`.
pub use otherwise_ui::Theme as Tokens;
pub use otherwise_ui::tokens::SANS;
use gpui::{App, Window};

pub fn tokens(cx: &App) -> Tokens {
    otherwise_ui::theme(cx)
}

/// `Local::theme`: "light", "dark", anything else follows the system.
pub fn apply(pref: Option<&str>, window: Option<&mut Window>, cx: &mut App) {
    otherwise_ui::apply(otherwise_ui::Appearance::parse(pref), window, cx);
}
