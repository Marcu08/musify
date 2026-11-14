use rune::tok::{self, type_};
use rune::{Color, TextStyle};

/// Musify's palette. Components read named roles from here; no component picks a
/// raw colour. The base roles come from Rune's dark theme tokens.
#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color,
    pub sidebar: Color,
    pub card: Color,
    pub text: Color,
    pub text_secondary: Color,
    pub separator: Color,
    pub accent: Color,
    pub accent_soft: Color,
    pub accent_hover: Color,
    pub danger: Color,
    pub control_track: Color,
    pub hover: Color,
}

pub fn palette() -> Palette {
    let theme = tok::theme(true);
    let accent = tok::accent(3, true);
    Palette {
        bg: theme.bg,
        sidebar: theme.card,
        card: theme.card,
        text: theme.text,
        text_secondary: theme.text_secondary,
        separator: theme.separator,
        accent,
        accent_soft: accent.alpha(0.14),
        accent_hover: accent.alpha(0.22),
        danger: tok::accent(2, true),
        control_track: theme.control_track,
        hover: theme.hover,
    }
}

pub const APP_TITLE: TextStyle = TextStyle::new(21.0, 700.0);
pub const PAGE_TITLE: TextStyle = TextStyle::new(27.0, 700.0);
pub const SECTION_TITLE: TextStyle = TextStyle::new(15.0, 600.0);
pub const ROW_TITLE: TextStyle = TextStyle::new(13.5, 600.0);
pub const ROW_META: TextStyle = TextStyle::new(12.0, 400.0);
pub const BODY: TextStyle = type_::BODY;
pub const BUTTON: TextStyle = TextStyle::new(12.0, 600.0);
pub const SMALL: TextStyle = TextStyle::new(11.0, 500.0);
pub const TINY: TextStyle = TextStyle::new(10.0, 500.0);
pub const PLAYER_TITLE: TextStyle = TextStyle::new(13.0, 600.0);
pub const PLAYER_META: TextStyle = TextStyle::new(11.5, 400.0);
pub const TIME: TextStyle = TextStyle::new(11.0, 500.0);

pub const SIDEBAR_W: f32 = 232.0;
pub const TOPBAR_H: f32 = 70.0;
pub const PLAYER_H: f32 = 92.0;
pub const ROW_H: f32 = 56.0;
pub const TRACK_COVER: f32 = 38.0;
pub const ALBUM_COVER: f32 = 160.0;
pub const ALBUM_CARD_H: f32 = 238.0;
pub const PLAYER_COVER: f32 = 56.0;
pub const RADIUS: f32 = 8.0;
pub const RADIUS_LG: f32 = 14.0;
pub const TRANSPARENT: Color = Color::TRANSPARENT;
