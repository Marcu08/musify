use rune::{El, col, el, row, spacer, text};

use crate::i18n;
use crate::state::{UiState, View};
use crate::theme::{self, palette};
use crate::view::common;

pub fn view(s: &UiState) -> El {
    let p = palette();
    let logo = row()
        .w_full()
        .px(12.0)
        .pt(8.0)
        .pb(18.0)
        .gap(9.0)
        .center_y()
        .child(el().size(11.0, 11.0).radius(5.5).fill(p.accent))
        .child(text(i18n::APP_NAME).font(theme::APP_TITLE).color(p.text));
    let tagline = text(i18n::APP_TAGLINE)
        .font(theme::TINY)
        .color(p.text_secondary.alpha(0.75))
        .px(12.0)
        .pb(14.0);

    let sources = nav_button(s, View::Sources, i18n::NAV_SOURCES);
    let tracks = nav_button(s, View::Tracks, i18n::NAV_TRACKS);
    let albums = nav_button(s, View::Albums, i18n::NAV_ALBUMS);
    let artists = nav_button(s, View::Artists, i18n::NAV_ARTISTS);
    let queue = nav_button(s, View::Queue, i18n::NAV_QUEUE);
    let settings = nav_button(s, View::Settings, i18n::NAV_SETTINGS);

    let status = if let Some(scan) = s.scan {
        let label = i18n::scan_progress(scan.scanned, scan.total);
        col()
            .w_full()
            .gap(5.0)
            .px(12.0)
            .py(8.0)
            .child(text(label).font(theme::TINY).color(p.text_secondary))
            .child(
                el().w_full()
                    .h(3.0)
                    .radius(1.5)
                    .fill(p.control_track)
                    .child(
                        el().w(if scan.total == 0 {
                            0.0
                        } else {
                            190.0 * (scan.scanned as f32 / scan.total as f32).clamp(0.0, 1.0)
                        })
                        .h(3.0)
                        .radius(1.5)
                        .fill(p.accent),
                    ),
            )
    } else if let Some(last) = &s.scan_last {
        col()
            .w_full()
            .px(12.0)
            .py(8.0)
            .gap(2.0)
            .child(
                text(i18n::SCAN_IDLE)
                    .font(theme::TINY)
                    .color(p.text_secondary),
            )
            .child(
                text(last.clone())
                    .font(theme::TINY)
                    .color(p.text_secondary.alpha(0.7)),
            )
    } else {
        col().w_full().px(12.0).py(8.0).child(
            text(i18n::tracks_count(s.stats.tracks as usize))
                .font(theme::TINY)
                .color(p.text_secondary),
        )
    };

    col()
        .w(theme::SIDEBAR_W)
        .h_full()
        .fill(p.sidebar)
        .padding(10.0)
        .gap(2.0)
        .child(logo)
        .child(tagline)
        .child(common::section_label(i18n::NAV_SOURCES))
        .child(sources)
        .child(common::section_label(i18n::NAV_LIBRARY))
        .child(tracks)
        .child(albums)
        .child(artists)
        .child(common::section_label(i18n::NAV_PLAYBACK))
        .child(queue)
        .child(spacer())
        .child(status)
        .child(common::divider())
        .child(el().h(4.0))
        .child(settings)
        .child(el().h(8.0))
}

fn nav_button(s: &UiState, view: View, label: &'static str) -> El {
    let item = common::nav_item(label, s.view == view);
    item.on_press(move |s: &mut UiState| s.set_view(view))
        .child(spacer())
        .child(
            text(match view {
                View::Tracks => s.stats.tracks.to_string(),
                View::Albums => s.stats.albums.to_string(),
                View::Artists => s.stats.artists.to_string(),
                View::Queue => s.queue.len().to_string(),
                View::Sources | View::Radio => String::new(),
                View::Settings => String::new(),
            })
            .font(theme::TINY)
            .color(palette().text_secondary),
        )
}
