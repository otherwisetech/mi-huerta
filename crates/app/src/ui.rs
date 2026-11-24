//! The shared building blocks come from otherwise-ui; only Mi Huerta's own mark lives here.
use gpui::*;

pub use otherwise_ui::{Tone, banner, card as panel, chip, filled_button, icon, link, outline_button, section_title, segment, segmented};

/// A label above a control; keeps a minimum width inside the forms' rows.
pub fn labelled(t: &crate::theme::Tokens, text: impl Into<SharedString>, control: impl IntoElement) -> Div {
    otherwise_ui::labelled(t, text, control).min_w(px(90.))
}

/// The app's own mark, tinted.
pub fn logo_glyph(size: f32, color: Hsla) -> Svg {
    otherwise_ui::glyph(crate::LOGO_SVG, size, color)
}
