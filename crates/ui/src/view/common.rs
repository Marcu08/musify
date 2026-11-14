use std::path::Path;

use rune::{Color, El, SurfaceFit, el, row, text};

use crate::i18n;
use crate::state::UiState;
use crate::theme::{self, Palette};

pub fn palette() -> Palette {
    theme::palette()
}

pub fn cover(s: &UiState, path: Option<&Path>, size: f32, radius: f32, label: &str) -> El {
    let p = palette();
    if let Some(image) = s.cover(path) {
        el().size(size, size)
            .radius(radius)
            .pixels_fit(image.pixels(), SurfaceFit::Fill)
    } else {
        let initial = label
            .chars()
            .find(|character| character.is_alphanumeric())
            .map_or_else(
                || "♪".to_string(),
                |character| character.to_uppercase().to_string(),
            );
        el().size(size, size)
            .radius(radius)
            .fill(p.accent_soft)
            .center()
            .child(text(initial).font(theme::SECTION_TITLE).color(p.accent))
    }
}

pub fn cover_wide(s: &UiState, path: Option<&Path>, height: f32, radius: f32, label: &str) -> El {
    let p = palette();
    if let Some(image) = s.cover(path) {
        el().w_full()
            .h(height)
            .radius(radius)
            .pixels_fit(image.pixels(), SurfaceFit::Fill)
    } else {
        let initial = label
            .chars()
            .find(|character| character.is_alphanumeric())
            .map_or_else(
                || "♪".to_string(),
                |character| character.to_uppercase().to_string(),
            );
        el().w_full()
            .h(height)
            .radius(radius)
            .fill(p.accent_soft)
            .center()
            .child(text(initial).font(theme::APP_TITLE).color(p.accent))
    }
}

pub fn nav_item(label: &'static str, active: bool) -> El {
    let p = palette();
    let mut item = row()
        .h(38.0)
        .w_full()
        .px(12.0)
        .gap(10.0)
        .center_y()
        .radius(theme::RADIUS)
        .fill(if active {
            p.accent_soft
        } else {
            Color::TRANSPARENT
        })
        .hover(p.hover);
    item = item.child(el().w(3.0).h(18.0).radius(1.5).fill(if active {
        p.accent
    } else {
        Color::TRANSPARENT
    }));
    item.child(
        text(label)
            .font(theme::BODY)
            .color(if active { p.text } else { p.text_secondary }),
    )
}

pub fn pill(label: impl Into<String>, active: bool) -> El {
    let p = palette();
    row()
        .h(32.0)
        .px(12.0)
        .radius(16.0)
        .center()
        .gap(4.0)
        .fill(if active {
            p.accent_soft
        } else {
            p.control_track
        })
        .hover(if active { p.accent_hover } else { p.hover })
        .child(text(label.into()).font(theme::BUTTON).color(if active {
            p.accent
        } else {
            p.text_secondary
        }))
}

/// A genre button in the Jamendo grid. It grows to fill its column so the rows
/// line up without hardcoded widths.
pub fn choice_chip(label: &'static str, active: bool) -> El {
    let p = palette();
    row()
        .h(26.0)
        .px(9.0)
        .radius(theme::RADIUS)
        .center()
        .fill(if active { p.accent_soft } else { p.card })
        .hover(if active { p.accent_hover } else { p.hover })
        .child(text(label).font(theme::BUTTON).color(if active {
            p.accent
        } else {
            p.text_secondary
        }))
}

pub fn action_button(label: impl Into<String>, accent: bool) -> El {
    let p = palette();
    row()
        .h(36.0)
        .px(14.0)
        .radius(18.0)
        .center()
        .fill(if accent { p.accent } else { p.control_track })
        .hover(if accent { p.accent_hover } else { p.hover })
        .child(text(label.into()).font(theme::BUTTON).color(if accent {
            Color::WHITE
        } else {
            p.text
        }))
}

pub fn small_action(label: impl Into<String>) -> El {
    let p = palette();
    row()
        .h(26.0)
        .px(8.0)
        .radius(6.0)
        .center()
        .hover(p.hover)
        .child(text(label.into()).font(theme::TINY).color(p.text_secondary))
}

pub fn section_label(label: &'static str) -> El {
    let p = palette();
    text(label)
        .font(theme::TINY)
        .color(p.text_secondary.alpha(0.8))
        .pt(theme::RADIUS)
}

pub fn divider() -> El {
    el().w_full().h(1.0).fill(palette().separator)
}

pub fn empty_state(title: &str, body: &str, action: Option<El>) -> El {
    let p = palette();
    let mut content = rune::col()
        .w_full()
        .h_full()
        .center()
        .gap(10.0)
        .child(text(title).font(theme::SECTION_TITLE).color(p.text))
        .child(
            text(body)
                .font(theme::ROW_META)
                .color(p.text_secondary)
                .max_w(420.0)
                .center(),
        );
    if let Some(action) = action {
        content = content.child(action);
    }
    content
}

pub fn unknown_artist(value: &str) -> String {
    if value.trim().is_empty() {
        i18n::UNKNOWN_ARTIST.into()
    } else {
        value.to_string()
    }
}

pub fn unknown_album(value: &str) -> String {
    if value.trim().is_empty() {
        i18n::UNKNOWN_ALBUM.into()
    } else {
        value.to_string()
    }
}

pub fn unknown_title(value: &str) -> String {
    if value.trim().is_empty() {
        i18n::UNKNOWN_TITLE.into()
    } else {
        value.to_string()
    }
}
