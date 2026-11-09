use rune::{El, FileDialogOptions, ScrollSpec, col, el, row, text};

use crate::i18n;
use crate::state::UiState;
use crate::theme::{self, palette};
use crate::view::common;

pub fn view(s: &UiState) -> El {
    let p = palette();
    let heading = text(i18n::SETTINGS_TITLE)
        .font(theme::PAGE_TITLE)
        .color(p.text);

    let folders = settings_card(i18n::FOLDERS_TITLE, i18n::FOLDERS_HINT, folders_body(s));
    let library = settings_card(i18n::LIBRARY_TITLE, "", library_body(s));
    let audio = settings_card(i18n::AUDIO_TITLE, "", audio_body(s));
    let shortcuts = settings_card(i18n::SHORTCUTS_TITLE, i18n::SHORTCUTS, el().h(1.0));

    let content = col()
        .w_full()
        .gap(16.0)
        .child(heading)
        .child(folders)
        .child(library)
        .child(audio)
        .child(shortcuts)
        .child(el().h(12.0));

    col().w_full().h_full().px(26.0).child(
        col()
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .child(content),
    )
}

fn settings_card(title: &'static str, description: &'static str, body: El) -> El {
    let p = palette();
    let mut card = col()
        .w_full()
        .gap(8.0)
        .padding(18.0)
        .radius(theme::RADIUS_LG)
        .fill(p.card)
        .child(text(title).font(theme::SECTION_TITLE).color(p.text));
    if !description.is_empty() {
        card = card.child(
            text(description)
                .font(theme::ROW_META)
                .color(p.text_secondary),
        );
    }
    card.child(el().h(4.0)).child(body)
}

fn folders_body(s: &UiState) -> El {
    let p = palette();
    let add = common::action_button(i18n::ADD_FOLDER, true).on_press_tx(
        |_: &mut UiState, tx: &mut rune::Tx| {
            let options = FileDialogOptions {
                title: i18n::ADD_FOLDER.to_string(),
                ..FileDialogOptions::default()
            };
            tx.open_folder(options, |s: &mut UiState, result, _tx| match result {
                Ok(Some(path)) => s.add_folder(path),
                Ok(None) => {}
                Err(error) => s.error = Some(error.to_string()),
            });
        },
    );

    let mut list = col().w_full().gap(2.0);
    if s.folders.is_empty() {
        list = list.child(
            text(i18n::EMPTY_FOLDERS)
                .font(theme::ROW_META)
                .color(p.text_secondary),
        );
    } else {
        for folder in &s.folders {
            let path = folder.clone();
            list = list.child(
                row()
                    .w_full()
                    .h(40.0)
                    .gap(10.0)
                    .center_y()
                    .radius(theme::RADIUS)
                    .hover(p.hover)
                    .child(
                        text(path.display().to_string())
                            .font(theme::ROW_META)
                            .color(p.text)
                            .grow(1.0)
                            .max_w(720.0),
                    )
                    .child(
                        common::small_action(i18n::REMOVE)
                            .on_press(move |s: &mut UiState| s.remove_folder(&path)),
                    ),
            );
        }
    }
    col().w_full().gap(10.0).child(list).child(row().child(add))
}

fn library_body(s: &UiState) -> El {
    let p = palette();
    let stats = format!(
        "{} · {} · {} · {}",
        i18n::tracks_count(s.stats.tracks as usize),
        i18n::albums_count(s.stats.albums as usize),
        i18n::artists_count(s.stats.artists as usize),
        i18n::duration_long(s.stats.total_duration)
    );
    let status = if let Some(scan) = s.scan {
        i18n::scan_progress(scan.scanned, scan.total)
    } else if let Some(last) = &s.scan_last {
        last.clone()
    } else {
        i18n::SCAN_IDLE.to_string()
    };
    row()
        .w_full()
        .gap(12.0)
        .center_y()
        .child(
            col()
                .grow(1.0)
                .gap(3.0)
                .child(text(stats).font(theme::ROW_META).color(p.text))
                .child(text(status).font(theme::TINY).color(p.text_secondary)),
        )
        .child(common::action_button(i18n::RESCAN, false).on_press(|s: &mut UiState| s.rescan()))
}

fn audio_body(s: &UiState) -> El {
    let p = palette();
    let label = if s.output.0 == 0 {
        i18n::AUDIO_UNAVAILABLE.to_string()
    } else {
        format!("{} Hz · {} canali", s.output.0, s.output.1)
    };
    text(label).font(theme::ROW_META).color(p.text_secondary)
}
