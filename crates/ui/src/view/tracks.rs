use rune::{Color, El, GrowthSpec, ScrollSpec, col, el, row, spacer, text};

use crate::i18n;
use crate::state::{UiState, View};
use crate::theme::{self, palette};
use crate::view::common;

pub fn view(s: &UiState) -> El {
    let p = palette();
    let header = header();
    let (title, subtitle) = if let Some(filter) = &s.filter {
        (filter.title(), filter.subtitle())
    } else {
        (
            i18n::TRACKS_TITLE.to_string(),
            i18n::tracks_count(s.tracks.len()),
        )
    };
    let mut heading = col().w_full().gap(4.0).child(
        row()
            .w_full()
            .gap(10.0)
            .center_y()
            .child(text(title).font(theme::PAGE_TITLE).color(p.text))
            .child(spacer())
            .child(
                common::action_button(i18n::PLAY_ALL, true)
                    .on_press(|s: &mut UiState| s.play_all()),
            ),
    );
    if s.filter.is_some() {
        heading = heading.child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    common::small_action(format!("‹ {}", i18n::BACK))
                        .on_press(|s: &mut UiState| s.back()),
                )
                .child(text(subtitle).font(theme::ROW_META).color(p.text_secondary)),
        );
    } else {
        heading = heading.child(text(subtitle).font(theme::ROW_META).color(p.text_secondary));
    }

    let list = if s.current_list().is_empty() {
        let action = common::action_button(i18n::OPEN_SETTINGS, true)
            .on_press(|s: &mut UiState| s.set_view(View::Settings));
        if s.filter.is_some() {
            common::empty_state(i18n::EMPTY_FILTER, "", Some(action))
        } else {
            common::empty_state(i18n::EMPTY_LIBRARY, i18n::EMPTY_LIBRARY_HINT, Some(action))
        }
    } else {
        let stack = col()
            .key("track-rows")
            .w_full()
            .growth(GrowthSpec::default())
            .virtual_rows(
                s.current_list().len(),
                theme::ROW_H,
                |s: &UiState, index| s.current_keys()[index].as_str().into(),
                |s: &UiState, index| track_row(s, index),
            )
            .virtual_rows_version(s.list_version);
        col()
            .key("track-list")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .child(stack)
    };

    col()
        .w_full()
        .h_full()
        .px(26.0)
        .gap(12.0)
        .child(heading)
        .child(header)
        .child(list)
        .child(el().h(10.0))
}

fn header() -> El {
    let p = palette();
    let label = |value: &'static str, width: f32| {
        text(value)
            .font(theme::TINY)
            .color(p.text_secondary.alpha(0.85))
            .w(width)
    };
    row()
        .w_full()
        .h(28.0)
        .px(10.0)
        .gap(12.0)
        .center_y()
        .child(text("#").font(theme::TINY).color(p.text_secondary).w(26.0))
        .child(el().w(theme::TRACK_COVER))
        .child(
            text(i18n::COL_TITLE)
                .font(theme::TINY)
                .color(p.text_secondary.alpha(0.85))
                .grow(1.0),
        )
        .child(label(i18n::COL_ALBUM, 190.0))
        .child(
            text(i18n::COL_DURATION)
                .font(theme::TINY)
                .color(p.text_secondary.alpha(0.85))
                .w(54.0),
        )
}

fn track_row(s: &UiState, index: usize) -> El {
    let p = palette();
    let Some(track) = s.current_list().get(index) else {
        return el().h(theme::ROW_H);
    };
    let current = s
        .current
        .as_ref()
        .is_some_and(|item| item.track.id == track.id)
        && s.playback != musify_core::PlaybackState::Stopped;
    let id = track.id;
    let title = common::unknown_title(&track.title);
    let artist = common::unknown_artist(&track.artist);
    let album = common::unknown_album(&track.album);
    let duration = if track.duration.is_zero() {
        "--:--".to_string()
    } else {
        i18n::duration_text(track.duration)
    };
    let row_el = row()
        .key(format!("track-{}", id.get()))
        .w_full()
        .h(theme::ROW_H)
        .px(10.0)
        .gap(12.0)
        .radius(theme::RADIUS)
        .center_y()
        .fill(if current {
            p.accent_soft
        } else {
            Color::TRANSPARENT
        })
        .hover(p.hover)
        .child(
            text(format!("{}", index + 1))
                .font(theme::TINY)
                .color(p.text_secondary)
                .w(26.0),
        )
        .child(common::cover(
            s,
            track.cover_path.as_deref(),
            theme::TRACK_COVER,
            5.0,
            &title,
        ))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(1.0)
                .child(text(title).font(theme::ROW_TITLE).color(if current {
                    p.accent
                } else {
                    p.text
                }))
                .child(text(artist).font(theme::ROW_META).color(p.text_secondary)),
        )
        .child(
            text(album)
                .font(theme::ROW_META)
                .color(p.text_secondary)
                .w(190.0),
        )
        .child(
            text(duration)
                .font(theme::TIME)
                .color(p.text_secondary)
                .w(54.0),
        );
    row_el.on_press(move |s: &mut UiState| s.play_track(id))
}
