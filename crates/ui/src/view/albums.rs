use rune::{El, GrowthSpec, ScrollSpec, col, el, row, spacer, text};

use crate::i18n;
use crate::state::UiState;
use crate::theme::{self, palette};
use crate::view::common;

const COLUMNS: usize = 4;

pub fn view(s: &UiState) -> El {
    let p = palette();
    let heading = row()
        .w_full()
        .gap(10.0)
        .center_y()
        .child(
            text(i18n::ALBUMS_TITLE)
                .font(theme::PAGE_TITLE)
                .color(p.text),
        )
        .child(spacer())
        .child(
            text(i18n::albums_count(s.albums.len()))
                .font(theme::ROW_META)
                .color(p.text_secondary),
        );
    if s.albums.is_empty() {
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

    let rows = s.albums.len().div_ceil(COLUMNS);
    let stack = col()
        .key("album-rows")
        .w_full()
        .growth(GrowthSpec::default())
        .virtual_rows(
            rows,
            theme::ALBUM_CARD_H,
            |_: &UiState, index| format!("album-row-{index}").into(),
            album_row,
        )
        .virtual_rows_version(s.list_version);
    let list = col()
        .key("album-list")
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

fn album_row(s: &UiState, row_index: usize) -> El {
    let mut row_el = row().w_full().h(theme::ALBUM_CARD_H).gap(16.0);
    for column in 0..COLUMNS {
        let Some(album) = s.albums.get(row_index * COLUMNS + column) else {
            row_el = row_el.child(col().grow(1.0));
            continue;
        };
        row_el = row_el.child(album_card(s, album));
    }
    row_el
}

fn album_card(s: &UiState, album: &musify_core::Album) -> El {
    let p = palette();
    let id = album.id;
    let title = common::unknown_title(&album.title);
    let artist = common::unknown_artist(&album.artist);
    let meta = i18n::album_meta_short(album.year, album.track_count);
    col()
        .key(format!("album-{}", id.get()))
        .grow(1.0)
        .min_w(0.0)
        .gap(7.0)
        .padding(9.0)
        .radius(theme::RADIUS_LG)
        .hover(p.hover)
        .on_press(move |s: &mut UiState| s.open_album(id))
        .child(common::cover_wide(
            s,
            album.cover_path.as_deref(),
            theme::ALBUM_COVER,
            10.0,
            &title,
        ))
        .child(text(title).font(theme::ROW_TITLE).color(p.text))
        .child(text(artist).font(theme::ROW_META).color(p.text_secondary))
        .child(
            text(meta)
                .font(theme::TINY)
                .color(p.text_secondary.alpha(0.7)),
        )
}
