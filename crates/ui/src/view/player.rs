use rune::{Color, Drag, El, Phase, col, el, row, text};

use crate::i18n;
use crate::state::UiState;
use crate::theme::{self, palette};
use crate::view::common;

const SEEK_W: f32 = 220.0;
const VOLUME_W: f32 = 90.0;

pub fn view(s: &UiState) -> El {
    let p = palette();
    let current_title = s.current.as_ref().map_or_else(
        || i18n::NO_TRACK.to_string(),
        |item| common::unknown_title(&item.track.title),
    );
    let current_artist = s.current.as_ref().map_or_else(
        || i18n::NO_TRACK_HINT.to_string(),
        |item| common::unknown_artist(&item.track.artist),
    );
    let cover = common::cover(
        s,
        s.current
            .as_ref()
            .and_then(|item| item.track.cover_path.as_deref()),
        theme::PLAYER_COVER,
        7.0,
        &current_title,
    );

    let left = row().w(280.0).gap(12.0).center_y().child(cover).child(
        col()
            .grow(1.0)
            .min_w(0.0)
            .gap(2.0)
            .child(text(current_title).font(theme::PLAYER_TITLE).color(p.text))
            .child(
                text(if current_artist.is_empty() {
                    i18n::NO_TRACK_HINT.to_string()
                } else {
                    current_artist
                })
                .font(theme::PLAYER_META)
                .color(p.text_secondary),
            )
            .child(
                text(i18n::state_label(s.playback))
                    .font(theme::TINY)
                    .color(p.accent),
            ),
    );

    let controls = row()
        .w_full()
        .gap(8.0)
        .center()
        .child(
            common::pill(i18n::SHUFFLE, s.shuffle).on_press(|s: &mut UiState| s.toggle_shuffle()),
        )
        .child(common::pill(i18n::PREVIOUS, false).on_press(|s: &mut UiState| s.previous()))
        .child(play_button(s))
        .child(common::pill(i18n::NEXT, false).on_press(|s: &mut UiState| s.next()))
        .child(
            common::pill(
                i18n::repeat_label(s.repeat),
                s.repeat != musify_core::RepeatMode::Off,
            )
            .on_press(|s: &mut UiState| s.cycle_repeat()),
        );

    let duration = s
        .duration
        .or_else(|| s.current.as_ref().map(|item| item.track.duration))
        .unwrap_or_default();
    let fraction = if duration.is_zero() {
        0.0
    } else {
        (s.position.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0)
    };
    let seek = col().w_full().gap(3.0).child(controls).child(
        row()
            .w_full()
            .gap(8.0)
            .center_y()
            .child(
                text(i18n::duration_text(s.position))
                    .font(theme::TIME)
                    .color(p.text_secondary)
                    .w(44.0),
            )
            .child(
                el().key("seek")
                    .w(SEEK_W)
                    .h(16.0)
                    .on_drag(|s: &mut UiState, drag: &mut Drag, _tx| {
                        if matches!(drag.phase, Phase::Down | Phase::Up) {
                            let fraction = (drag.x / drag.w.max(1.0)).clamp(0.0, 1.0);
                            s.seek_fraction(fraction);
                        }
                    })
                    .child(
                        el().w(SEEK_W)
                            .h(4.0)
                            .radius(2.0)
                            .fill(p.control_track)
                            .child(
                                el().w((SEEK_W * fraction).max(0.0))
                                    .h(4.0)
                                    .radius(2.0)
                                    .fill(p.accent),
                            ),
                    ),
            )
            .child(
                text(i18n::duration_text(duration))
                    .font(theme::TIME)
                    .color(p.text_secondary)
                    .w(44.0),
            ),
    );

    let center = col().grow(1.0).min_w(0.0).gap(4.0).center().child(seek);

    let volume_fraction = f32::from(s.volume) / 100.0;
    let right = row()
        .w(230.0)
        .gap(10.0)
        .center_y()
        .child(text(i18n::VOLUME).font(theme::TINY).color(p.text_secondary))
        .child(
            el().key("volume")
                .w(VOLUME_W)
                .h(16.0)
                .on_drag(|s: &mut UiState, drag: &mut Drag, _tx| {
                    if matches!(drag.phase, Phase::Down | Phase::Up) {
                        let fraction = (drag.x / drag.w.max(1.0)).clamp(0.0, 1.0);
                        s.set_volume((fraction * 100.0).round() as u16);
                    }
                })
                .child(
                    el().w(VOLUME_W)
                        .h(4.0)
                        .radius(2.0)
                        .fill(p.control_track)
                        .child(
                            el().w((VOLUME_W * volume_fraction).max(0.0))
                                .h(4.0)
                                .radius(2.0)
                                .fill(p.accent),
                        ),
                ),
        )
        .child(
            common::small_action(i18n::QUEUE)
                .on_press(|s: &mut UiState| s.set_view(crate::state::View::Queue)),
        );

    col().w_full().child(common::divider()).child(
        row()
            .h(theme::PLAYER_H)
            .w_full()
            .px(18.0)
            .gap(18.0)
            .center_y()
            .fill(p.card)
            .child(left)
            .child(center)
            .child(right),
    )
}

fn play_button(s: &UiState) -> El {
    let p = palette();
    let label = match s.playback {
        musify_core::PlaybackState::Playing => i18n::PAUSE,
        musify_core::PlaybackState::Paused => i18n::RESUME,
        musify_core::PlaybackState::Stopped => i18n::PLAY,
    };
    row()
        .h(38.0)
        .px(18.0)
        .radius(19.0)
        .center()
        .fill(p.accent)
        .hover(p.accent_hover)
        .on_press(|s: &mut UiState| s.toggle_play())
        .child(text(label).font(theme::BUTTON).color(Color::WHITE))
}
