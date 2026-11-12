use rune::{El, ScrollSpec, col, el, row, spacer, text};

use crate::i18n;
use crate::state::UiState;
use crate::theme::{self, palette};
use crate::view::common;

pub fn view(s: &UiState) -> El {
    let p = palette();
    let Some(results) = &s.search else {
        return common::empty_state(i18n::SEARCH_HINT, "", None);
    };
    let tracks = results.tracks.clone();
    let albums = results.albums.clone();
    let artists = results.artists.clone();
    if tracks.is_empty() && albums.is_empty() && artists.is_empty() {
        return common::empty_state(i18n::SEARCH_EMPTY, i18n::SEARCH_HINT, None);
    }

    let mut content = col().w_full().gap(14.0).child(
        text(i18n::results_for(s.query.text.trim()))
            .font(theme::PAGE_TITLE)
            .color(p.text),
    );

    if !tracks.is_empty() {
        content = content.child(section(
            i18n::TRACKS_TITLE,
            i18n::tracks_count(tracks.len()),
        ));
        let mut list = col().w_full().gap(2.0);
        for track in &tracks {
            list = list.child(track_row(s, track));
        }
        content = content.child(list);
    }
    if !albums.is_empty() {
        content = content.child(section(
            i18n::ALBUMS_TITLE,
            i18n::albums_count(albums.len()),
        ));
        let mut list = col().w_full().gap(2.0);
        for album in &albums {
            list = list.child(album_row(s, album));
        }
        content = content.child(list);
    }
    if !artists.is_empty() {
        content = content.child(section(
            i18n::ARTISTS_TITLE,
            i18n::artists_count(artists.len()),
        ));
        let mut list = col().w_full().gap(2.0);
        for artist in &artists {
            list = list.child(artist_row(s, artist));
        }
        content = content.child(list);
    }

    col().w_full().h_full().px(26.0).child(
        col()
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .child(content)
            .child(el().h(10.0)),
    )
}

fn section(title: &'static str, count: String) -> El {
    let p = palette();
    row()
        .w_full()
        .pt(6.0)
        .gap(10.0)
        .center_y()
        .child(text(title).font(theme::SECTION_TITLE).color(p.text))
        .child(text(count).font(theme::TINY).color(p.text_secondary))
        .child(spacer())
}

fn track_row(s: &UiState, track: &musify_core::Track) -> El {
    let p = palette();
    let title = common::unknown_title(&track.title);
    let artist = common::unknown_artist(&track.artist);
    let album = common::unknown_album(&track.album);
    let duration = if track.duration.is_zero() {
        "--:--".to_string()
    } else {
        i18n::duration_text(track.duration)
    };
    let value = track.clone();
    let add_track = track.clone();
    row()
        .key(format!("search-track-{}", track.id.get()))
        .w_full()
        .h(52.0)
        .px(8.0)
        .gap(10.0)
        .radius(theme::RADIUS)
        .center_y()
        .hover(p.hover)
        .on_press(move |s: &mut UiState| s.play_track_value(value.clone()))
        .child(common::cover(
            s,
            track.cover_path.as_deref(),
            36.0,
            5.0,
            &title,
        ))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(1.0)
                .child(text(title).font(theme::ROW_TITLE).color(p.text))
                .child(text(artist).font(theme::ROW_META).color(p.text_secondary)),
        )
        .child(
            text(album)
                .font(theme::ROW_META)
                .color(p.text_secondary)
                .w(200.0),
        )
        .child(text(duration).font(theme::TIME).color(p.text_secondary))
        .child(
            common::small_action(i18n::PLAYLIST_ADD).on_press(move |s: &mut UiState| {
                s.add_track_to_selected_playlist(add_track.clone());
            }),
        )
}

fn album_row(s: &UiState, album: &musify_core::Album) -> El {
    let p = palette();
    let title = common::unknown_title(&album.title);
    let meta = i18n::album_meta(&album.artist, album.year, album.track_count);
    let value = album.clone();
    row()
        .key(format!("search-album-{}", album.id.get()))
        .w_full()
        .h(56.0)
        .px(8.0)
        .gap(10.0)
        .radius(theme::RADIUS)
        .center_y()
        .hover(p.hover)
        .on_press(move |s: &mut UiState| s.open_album_value(value.clone()))
        .child(common::cover(
            s,
            album.cover_path.as_deref(),
            40.0,
            6.0,
            &title,
        ))
        .child(
            col()
                .grow(1.0)
                .gap(1.0)
                .child(text(title).font(theme::ROW_TITLE).color(p.text))
                .child(text(meta).font(theme::ROW_META).color(p.text_secondary)),
        )
}

fn artist_row(_s: &UiState, artist: &musify_core::Artist) -> El {
    let p = palette();
    let name = common::unknown_artist(&artist.name);
    let meta = i18n::artist_meta(artist.album_count, artist.track_count);
    let value = artist.clone();
    let initial = name
        .chars()
        .find(|character| character.is_alphanumeric())
        .map_or_else(
            || "♪".to_string(),
            |character| character.to_uppercase().to_string(),
        );
    row()
        .key(format!("search-artist-{}", artist.id.get()))
        .w_full()
        .h(56.0)
        .px(8.0)
        .gap(10.0)
        .radius(theme::RADIUS)
        .center_y()
        .hover(p.hover)
        .on_press(move |s: &mut UiState| s.open_artist_value(value.clone()))
        .child(
            el().size(40.0, 40.0)
                .radius(20.0)
                .fill(p.accent_soft)
                .center()
                .child(text(initial).font(theme::SECTION_TITLE).color(p.accent)),
        )
        .child(
            col()
                .grow(1.0)
                .gap(1.0)
                .child(text(name).font(theme::ROW_TITLE).color(p.text))
                .child(text(meta).font(theme::ROW_META).color(p.text_secondary)),
        )
}
