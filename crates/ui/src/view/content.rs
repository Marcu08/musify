use rune::{El, col, row, text};

use crate::i18n;
use crate::state::{UiState, View};
use crate::theme::{self, palette};
use crate::view::{
    albums, artists, common, jamendo, playlists, podcast, queue, radio, search, settings, sources,
    tracks,
};

pub fn view(s: &UiState) -> El {
    let content = if !s.query.text.trim().is_empty() {
        search::view(s)
    } else {
        match s.view {
            View::Tracks => tracks::view(s),
            View::Albums => albums::view(s),
            View::Artists => artists::view(s),
            View::Playlists => playlists::view(s),
            View::Queue => queue::view(s),
            View::Sources => sources::view(s),
            View::Radio => radio::view(s),
            View::Podcast => podcast::view(s),
            View::Jamendo => jamendo::view(s),
            View::Settings => settings::view(s),
        }
    };
    let mut column = col().w_full().h_full().min_w(0.0);
    if let Some(error) = &s.error {
        column = column.child(error_banner(error));
    }
    column.child(content)
}

fn error_banner(error: &str) -> El {
    let p = palette();
    row()
        .w_full()
        .h(38.0)
        .px(26.0)
        .gap(10.0)
        .center_y()
        .fill(p.danger.alpha(0.18))
        .child(
            text(error.to_string())
                .font(theme::SMALL)
                .color(p.text)
                .grow(1.0),
        )
        .child(common::small_action(i18n::CLOSE).on_press(|s: &mut UiState| s.clear_error()))
}
