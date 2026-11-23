//! Small styled building blocks shared by every tab.
use crate::theme::Tokens;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{Icon, IconName};

pub fn icon(name: IconName, size: f32, color: Hsla) -> Icon {
    Icon::new(name).size(px(size)).text_color(color)
}

/// Outlined 30 px control.
pub fn outline_button(t: &Tokens, id: impl Into<ElementId>) -> Stateful<Div> {
    let hover = t.border_strong;
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .h(px(30.))
        .px(px(11.))
        .rounded(px(8.))
        .border_1()
        .border_color(t.border)
        .bg(t.surface)
        .text_color(t.ink)
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .hover(move |s| s.border_color(hover))
}

/// Filled 30 px control.
pub fn filled_button(t: &Tokens, id: impl Into<ElementId>, bg: Hsla) -> Stateful<Div> {
    let hover = bg.opacity(0.88);
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .h(px(30.))
        .px(px(13.))
        .rounded(px(8.))
        .bg(bg)
        .text_color(t.on_fill())
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
}

/// A quiet text link.
pub fn link(t: &Tokens, id: impl Into<ElementId>) -> Stateful<Div> {
    div().id(id).text_color(t.accent).cursor_pointer().hover(|s| s.underline())
}

pub fn section_title(t: &Tokens, text: impl Into<SharedString>) -> Div {
    div().text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(t.ink_muted).child(text.into())
}

/// Rounded label with a tinted background.
pub fn chip(text: impl Into<SharedString>, fg: Hsla, bg: Hsla) -> Div {
    div().flex().flex_none().items_center().gap(px(4.)).h(px(20.)).px(px(7.)).rounded(px(6.)).bg(bg).text_color(fg).text_size(px(11.)).font_weight(FontWeight::MEDIUM).child(text.into())
}

/// A white rounded card.
pub fn panel(t: &Tokens) -> Div {
    div().flex().flex_col().gap(px(10.)).p(px(16.)).rounded(px(12.)).border_1().border_color(t.border).bg(t.surface)
}

/// One option of a segmented control.
pub fn segment(t: &Tokens, id: impl Into<ElementId>, on: bool) -> Stateful<Div> {
    let muted = t.ink_muted;
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .gap(px(5.))
        .h(px(26.))
        .px(px(10.))
        .rounded(px(7.))
        .text_size(px(12.))
        .cursor_pointer()
        .when(on, |d| d.bg(t.surface).font_weight(FontWeight::SEMIBOLD).shadow_sm())
        .when(!on, |d| d.text_color(muted))
}

pub fn segmented(t: &Tokens) -> Div {
    div().flex().flex_none().items_center().gap(px(2.)).p(px(2.)).rounded(px(9.)).bg(t.border.opacity(0.6))
}

/// A label above a control.
pub fn labelled(t: &Tokens, text: impl Into<SharedString>, control: impl IntoElement) -> Div {
    div().flex().flex_col().gap(px(4.)).min_w(px(90.)).child(div().text_size(px(12.)).text_color(t.ink_muted).child(text.into())).child(control)
}

/// A note / warning band.
pub fn banner(t: &Tokens, text: impl Into<SharedString>, warn: bool) -> Div {
    let (fg, bg) = if warn { (t.warn, t.warn_soft) } else { (t.ink_muted, t.raised) };
    div()
        .flex()
        .items_start()
        .gap(px(8.))
        .px(px(12.))
        .py(px(8.))
        .rounded(px(10.))
        .bg(bg)
        .text_size(px(12.))
        .line_height(px(17.))
        .text_color(if warn { t.ink } else { t.ink_muted })
        .child(div().pt(px(1.)).child(icon(if warn { IconName::TriangleAlert } else { IconName::Info }, 14., fg)))
        .child(div().flex_1().min_w_0().child(text.into()))
}

/// The app's own mark, tinted.
pub fn logo_glyph(size: f32, color: Hsla) -> Svg {
    svg().path(crate::LOGO_SVG).flex_none().size(px(size)).text_color(color)
}
