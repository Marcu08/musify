use rune::{Color, El, GrowthSpec, ScrollSpec, col, el, row, spacer, text};

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
            text(i18n::QUEUE_TITLE)
                .font(theme::PAGE_TITLE)
                .color(p.text),
        )
        .child(spacer())
        .child(
            text(i18n::queue_count(s.queue.len()))
                .font(theme::ROW_META)
                .color(p.text_secondary),
        )
        .child(
            common::action_button(i18n::CLEAR_QUEUE, false)
                .on_press(|s: &mut UiState| s.clear_queue()),
        );

    if s.queue.is_empty() {
        return col()
            .w_full()
            .h_full()
            .px(26.0)
            .gap(12.0)
            .child(heading)
            .child(common::empty_state(
                i18n::EMPTY_QUEUE,
                i18n::EMPTY_QUEUE_HINT,
                None,
            ));
    }

    let stack = col()
        .key("queue-rows")
        .w_full()
        .growth(GrowthSpec::default())
        .virtual_rows(
            s.queue.len(),
            58.0,
            |s: &UiState, index| s.queue_keys[index].as_str().into(),
            |s: &UiState, index| queue_row(s, index),
        )
        .virtual_rows_version(s.list_version);
    let list = col()
        .key("queue-list")
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

fn queue_row(s: &UiState, index: usize) -> El {
    let p = palette();
    let Some(item) = s.queue.get(index) else {
        return el().h(58.0);
    };
    let current = s
        .current
        .as_ref()
        .is_some_and(|current| current.id == item.id)
        && s.playback != musify_core::PlaybackState::Stopped;
    let id = item.id;
    let track = item.track.clone();
    let title = common::unknown_title(&track.title);
    let artist = common::unknown_artist(&track.artist);
    let duration = if track.duration.is_zero() {
        "--:--".to_string()
    } else {
        i18n::duration_text(track.duration)
    };
    row()
        .key(format!("queue-{}", id.get()))
        .w_full()
        .h(58.0)
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
                .w(24.0),
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
                .on_press(move |s: &mut UiState| s.play_track_value(track.clone()))
                .child(text(title).font(theme::ROW_TITLE).color(if current {
                    p.accent
                } else {
                    p.text
                }))
                .child(text(artist).font(theme::ROW_META).color(p.text_secondary)),
        )
        .child(
            text(duration)
                .font(theme::TIME)
                .color(p.text_secondary)
                .w(48.0),
        )
        .child(
            common::small_action(i18n::UP).on_press(move |s: &mut UiState| s.move_queue_id(id, -1)),
        )
        .child(
            common::small_action(i18n::DOWN)
                .on_press(move |s: &mut UiState| s.move_queue_id(id, 1)),
        )
        .child(
            common::small_action(i18n::REMOVE)
                .on_press(move |s: &mut UiState| s.remove_queue_id(id)),
        )
}
