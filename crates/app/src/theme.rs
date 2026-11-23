//! Design tokens (the Otherwise family's warm neutrals) with Mi Huerta's leaf green, plus the
//! light / dark / follow-the-system switch.
use gpui::{App, Hsla, Window, rgb};
use gpui_component::{ActiveTheme, Theme, ThemeMode};

#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    pub canvas: Hsla,
    pub surface: Hsla,
    pub raised: Hsla,
    pub border: Hsla,
    pub border_strong: Hsla,
    pub ink: Hsla,
    pub ink_muted: Hsla,
    pub accent: Hsla,
    pub accent_soft: Hsla,
    pub danger: Hsla,
    pub danger_soft: Hsla,
    pub ok: Hsla,
    pub warn: Hsla,
    pub warn_soft: Hsla,
    pub rain: Hsla,
    pub rain_soft: Hsla,
    pub brand: Hsla,
    pub dark: bool,
}

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

fn a(hex: u32, alpha: f32) -> Hsla {
    let mut h = c(hex);
    h.a = alpha;
    h
}

impl Tokens {
    pub fn light() -> Self {
        Tokens {
            canvas: c(0xF3F2EC),
            surface: c(0xFFFFFF),
            raised: c(0xF8F8F3),
            border: c(0xE1E0D6),
            border_strong: c(0xC7C6B9),
            ink: c(0x1C1D18),
            ink_muted: c(0x686A5E),
            accent: c(0x2F7D3B),
            accent_soft: c(0xE3F0E1),
            danger: c(0xA3382A),
            danger_soft: a(0xD0503F, 0.10),
            ok: c(0x2F7D3B),
            warn: c(0x9A5B0C),
            warn_soft: a(0xF2A93B, 0.18),
            rain: c(0x2F6FA8),
            rain_soft: a(0x4A8FD0, 0.13),
            brand: c(0x2E7A3A),
            dark: false,
        }
    }

    pub fn dark() -> Self {
        Tokens {
            canvas: c(0x141513),
            surface: c(0x1C1D1A),
            raised: c(0x232420),
            border: c(0x30322C),
            border_strong: c(0x4A4C43),
            ink: c(0xF1F2EA),
            ink_muted: c(0xA0A295),
            accent: c(0x7FCB83),
            accent_soft: a(0x5DB865, 0.18),
            danger: c(0xE07767),
            danger_soft: a(0xE5675A, 0.14),
            ok: c(0x7FCB83),
            warn: c(0xF2B95A),
            warn_soft: a(0xF2A93B, 0.16),
            rain: c(0x7DB6EA),
            rain_soft: a(0x5FA3E0, 0.16),
            brand: c(0x2E7A3A),
            dark: true,
        }
    }

    pub fn on_fill(&self) -> Hsla {
        if self.dark { c(0x141513) } else { c(0xFFFFFF) }
    }
}

pub fn tokens(cx: &App) -> Tokens {
    if cx.theme().mode.is_dark() { Tokens::dark() } else { Tokens::light() }
}

/// `Local::theme`: "light", "dark", anything else follows the system.
pub fn apply(pref: Option<&str>, window: Option<&mut Window>, cx: &mut App) {
    match pref {
        Some("light") => Theme::change(ThemeMode::Light, window, cx),
        Some("dark") => Theme::change(ThemeMode::Dark, window, cx),
        _ => Theme::sync_system_appearance(window, cx),
    }
    Theme::global_mut(cx).font_family = SANS.into();
}

pub const SANS: &str = "Inter";
