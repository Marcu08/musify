use rune::{El, GrowthSpec, ScrollSpec, col, el, row, text};

use crate::i18n;
use crate::state::UiState;
use crate::theme::{self, palette};
use crate::view::common;

pub fn view(s: &UiState) -> El {
    let p = palette();
    let heading = row()
        .w_full()
        .gap(10.0)
        .center_y()
        .child(
            text(i18n::ARTISTS_TITLE)
                .font(theme::PAGE_TITLE)
                .color(p.text),
        )
        .child(rune::spacer())
        .child(
            text(i18n::artists_count(s.artists.len()))
                .font(theme::ROW_META)
                .color(p.text_secondary),
        );
    if s.artists.is_empty() {
        return col()
            .w_full()
            .h_full()
            .px(26.0)
            .gap(12.0)
            .child(heading)
            .child(common::empty_state(
                i18n::EMPTY_LIBRARY,
                i18n::EMPTY_LIBRARY_HINT,
                None,
            ));
    }

    let stack = col()
        .key("artist-rows")
        .w_full()
        .growth(GrowthSpec::default())
        .virtual_rows(
            s.artists.len(),
            64.0,
            |s: &UiState, index| s.artist_keys[index].as_str().into(),
            |s: &UiState, index| artist_row(s, index),
        )
        .virtual_rows_version(s.list_version);
    let list = col()
        .key("artist-list")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(stack);

    col()
        .w_full()
        .h_full()
        .px(26.0)
        .gap(12.0)
        .child(heading)
        .child(list)
        .child(el().h(10.0))
}

fn artist_row(s: &UiState, index: usize) -> El {
    let p = palette();
    let Some(artist) = s.artists.get(index) else {
        return el().h(64.0);
    };
    let id = artist.id;
    let name = common::unknown_artist(&artist.name);
    let initial = name
        .chars()
        .find(|character| character.is_alphanumeric())
        .map_or_else(
            || "♪".to_string(),
            |character| character.to_uppercase().to_string(),
        );
    let meta = i18n::artist_meta(artist.album_count, artist.track_count);
    row()
        .key(format!("artist-{}", id.get()))
        .w_full()
        .h(64.0)
        .px(10.0)
        .gap(12.0)
        .radius(theme::RADIUS)
        .center_y()
        .hover(p.hover)
        .on_press(move |s: &mut UiState| s.open_artist(id))
        .child(
            el().size(44.0, 44.0)
                .radius(22.0)
                .fill(p.accent_soft)
                .center()
                .child(text(initial).font(theme::SECTION_TITLE).color(p.accent)),
        )
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(2.0)
                .child(text(name).font(theme::ROW_TITLE).color(p.text))
                .child(text(meta).font(theme::ROW_META).color(p.text_secondary)),
        )
        .child(text("›").font(theme::SECTION_TITLE).color(p.text_secondary))
}
