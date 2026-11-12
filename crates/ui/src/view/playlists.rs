use rune::{
    Color, El, FileDialogOptions, GrowthSpec, ScrollSpec, col, el, row, spacer, text, text_input,
};

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
            text(i18n::PLAYLISTS_TITLE)
                .font(theme::PAGE_TITLE)
                .color(p.text),
        )
        .child(
            text(i18n::playlists_count(s.playlists.len()))
                .font(theme::ROW_META)
                .color(p.text_secondary),
        )
        .child(spacer())
        .child(import_button());

    col()
        .w_full()
        .h_full()
        .px(26.0)
        .gap(10.0)
        .child(heading)
        .child(
            text(i18n::PLAYLISTS_HINT)
                .font(theme::ROW_META)
                .color(p.text_secondary),
        )
        .child(
            row()
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .gap(14.0)
                .child(sidebar(s))
                .child(detail(s)),
        )
        .child(el().h(8.0))
}

fn sidebar(s: &UiState) -> El {
    let p = palette();
    let name = text_input(&s.playlist_name, theme::BODY)
        .key("playlist-name")
        .single_line()
        .h(36.0)
        .grow(1.0)
        .fill(p.card)
        .radius(8.0)
        .padding(9.0)
        .color(p.text)
        .placeholder(i18n::PLAYLIST_NAME, p.text_secondary)
        .on_edit(|s: &mut UiState| &mut s.playlist_name);
    let create = common::action_button(i18n::PLAYLIST_CREATE, true)
        .on_press(|s: &mut UiState| s.create_playlist());

    let mut list = col().w_full().gap(2.0);
    if s.playlists.is_empty() {
        list = list
            .child(
                text(i18n::PLAYLIST_EMPTY)
                    .font(theme::ROW_META)
                    .color(p.text_secondary),
            )
            .child(
                text(i18n::PLAYLIST_EMPTY_HINT)
                    .font(theme::TINY)
                    .color(p.text_secondary),
            );
    } else {
        for playlist in &s.playlists {
            let id = playlist.id;
            let selected = s.selected_playlist == Some(id);
            list = list.child(
                row()
                    .key(format!("playlist-{}", id.get()))
                    .w_full()
                    .h(44.0)
                    .px(10.0)
                    .gap(8.0)
                    .center_y()
                    .radius(theme::RADIUS)
                    .fill(if selected {
                        p.accent_soft
                    } else {
                        Color::TRANSPARENT
                    })
                    .hover(p.hover)
                    .on_press(move |s: &mut UiState| s.open_playlist(id))
                    .child(
                        text(playlist.name.clone())
                            .font(theme::ROW_TITLE)
                            .color(if selected { p.accent } else { p.text })
                            .grow(1.0),
                    )
                    .child(
                        text(playlist.item_count.to_string())
                            .font(theme::TINY)
                            .color(p.text_secondary),
                    ),
            );
        }
    }

    let save_queue = common::action_button(i18n::PLAYLIST_SAVE_QUEUE, false)
        .on_press(|s: &mut UiState| s.save_queue_as_playlist());

    col()
        .w(270.0)
        .h_full()
        .gap(10.0)
        .child(row().w_full().gap(8.0).center_y().child(name).child(create))
        .child(common::divider())
        .child(
            col()
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(list),
        )
        .child(save_queue)
}

fn detail(s: &UiState) -> El {
    let p = palette();
    if s.selected_playlist.is_none() {
        return col().grow(1.0).min_w(0.0).child(common::empty_state(
            i18n::PLAYLIST_SELECT,
            i18n::PLAYLIST_SELECT_HINT,
            None,
        ));
    }

    let play = common::action_button(i18n::PLAYLIST_PLAY, true)
        .on_press(|s: &mut UiState| s.play_selected_playlist());
    let enqueue = common::action_button(i18n::PLAYLIST_ENQUEUE, false)
        .on_press(|s: &mut UiState| s.enqueue_selected_playlist());
    let rename = common::small_action(i18n::PLAYLIST_RENAME)
        .on_press(|s: &mut UiState| s.rename_selected_playlist());
    let delete = common::small_action(i18n::PLAYLIST_DELETE)
        .on_press(|s: &mut UiState| s.delete_selected_playlist());
    let add_current = common::small_action(i18n::PLAYLIST_ADD_CURRENT)
        .on_press(|s: &mut UiState| s.add_current_to_selected_playlist());
    let add_queue = common::small_action(i18n::PLAYLIST_ADD_QUEUE)
        .on_press(|s: &mut UiState| s.add_queue_to_selected_playlist());
    let remove_missing = common::small_action(i18n::PLAYLIST_REMOVE_MISSING)
        .on_press(|s: &mut UiState| s.remove_missing_playlist_items());

    let mut head = col()
        .w_full()
        .gap(6.0)
        .child(
            row()
                .w_full()
                .gap(10.0)
                .center_y()
                .child(
                    text(s.selected_playlist_name.clone())
                        .font(theme::SECTION_TITLE)
                        .color(p.text),
                )
                .child(
                    text(i18n::playlist_items_count(s.playlist_items.len()))
                        .font(theme::ROW_META)
                        .color(p.text_secondary),
                )
                .child(spacer())
                .child(play)
                .child(enqueue),
        )
        .child(
            row()
                .w_full()
                .gap(6.0)
                .center_y()
                .child(rename)
                .child(delete)
                .child(export_button()),
        )
        .child(
            row()
                .w_full()
                .gap(6.0)
                .center_y()
                .child(add_current)
                .child(add_queue)
                .child(remove_missing),
        )
        .child(
            text(i18n::PLAYLIST_REORDER_HINT)
                .font(theme::TINY)
                .color(p.text_secondary),
        );
    if let Some(status) = &s.playlist_status {
        head = head.child(text(status.clone()).font(theme::TINY).color(p.accent));
    }

    let list = if s.playlist_items.is_empty() {
        common::empty_state(i18n::PLAYLIST_NO_ITEMS, i18n::PLAYLIST_NO_ITEMS_HINT, None)
    } else {
        let stack = col()
            .key("playlist-item-rows")
            .w_full()
            .growth(GrowthSpec::default())
            .virtual_rows(
                s.playlist_items.len(),
                theme::ROW_H,
                |s: &UiState, index| s.playlist_item_keys[index].as_str().into(),
                |s: &UiState, index| item_row(s, index),
            )
            .virtual_rows_version(s.list_version);
        col()
            .key("playlist-item-list")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .child(stack)
    };

    col()
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .gap(10.0)
        .child(head)
        .child(common::divider())
        .child(list)
}

fn item_row(s: &UiState, index: usize) -> El {
    let p = palette();
    let Some(item) = s.playlist_items.get(index) else {
        return el().h(theme::ROW_H);
    };
    let selected = s.playlist_selected_item == Some(item.id);
    let id = item.id;
    let title = common::unknown_title(&item.title);
    let artist = common::unknown_artist(&item.artist);
    let duration = if item.duration.is_zero() {
        "--:--".to_string()
    } else {
        i18n::duration_text(item.duration)
    };
    let available = item.available;
    let meta = if available {
        artist
    } else {
        format!("{artist} · {}", i18n::PLAYLIST_MISSING)
    };
    let text_color = if available {
        p.text
    } else {
        p.text_secondary.alpha(0.6)
    };
    let meta_color = if available {
        p.text_secondary
    } else {
        p.text_secondary.alpha(0.45)
    };
    let mut row_el = row()
        .key(format!("playlist-item-{}", id.get()))
        .w_full()
        .h(theme::ROW_H)
        .px(10.0)
        .gap(8.0)
        .radius(theme::RADIUS)
        .center_y()
        .fill(if selected {
            p.accent_soft
        } else {
            Color::TRANSPARENT
        })
        .hover(p.hover)
        .child(
            text(format!("{}", index + 1))
                .font(theme::TINY)
                .color(meta_color)
                .w(24.0),
        )
        .child(common::cover(
            s,
            item.cover_path.as_deref(),
            theme::TRACK_COVER,
            5.0,
            &title,
        ))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(1.0)
                .child(text(title).font(theme::ROW_TITLE).color(text_color))
                .child(text(meta).font(theme::ROW_META).color(meta_color)),
        )
        .child(text(duration).font(theme::TIME).color(meta_color).w(44.0));
    row_el = row_el
        .child(
            common::small_action(i18n::PLAYLIST_MOVE_UP)
                .on_press(move |s: &mut UiState| s.move_playlist_item(id, -1)),
        )
        .child(
            common::small_action(i18n::PLAYLIST_MOVE_DOWN)
                .on_press(move |s: &mut UiState| s.move_playlist_item(id, 1)),
        )
        .child(
            common::small_action(i18n::PLAYLIST_REMOVE_ITEM)
                .on_press(move |s: &mut UiState| s.remove_playlist_item(id)),
        );
    row_el.on_press(move |s: &mut UiState| s.select_playlist_item(id))
}

fn import_button() -> El {
    common::action_button(i18n::PLAYLIST_IMPORT, false).on_press_tx(
        |_: &mut UiState, tx: &mut rune::Tx| {
            let options = FileDialogOptions {
                title: i18n::PLAYLIST_IMPORT.to_string(),
                ..FileDialogOptions::default()
            }
            .filter("M3U8", &["m3u8", "m3u"]);
            tx.open_file(options, |s: &mut UiState, result, _tx| match result {
                Ok(Some(path)) => s.import_playlist(&path),
                Ok(None) => {}
                Err(error) => s.error = Some(error.to_string()),
            });
        },
    )
}

fn export_button() -> El {
    common::action_button(i18n::PLAYLIST_EXPORT, false).on_press_tx(
        |s: &mut UiState, tx: &mut rune::Tx| {
            if s.selected_playlist.is_none() {
                s.error = Some(i18n::PLAYLIST_SELECT.into());
                return;
            }
            let name = if s.selected_playlist_name.trim().is_empty() {
                "playlist".to_string()
            } else {
                s.selected_playlist_name.trim().to_string()
            };
            let options = FileDialogOptions {
                title: i18n::PLAYLIST_EXPORT.to_string(),
                file_name: Some(format!("{name}.m3u8")),
                ..FileDialogOptions::default()
            }
            .filter("M3U8", &["m3u8"]);
            tx.save_file(options, |s: &mut UiState, result, _tx| match result {
                Ok(Some(path)) => s.export_playlist(&path),
                Ok(None) => {}
                Err(error) => s.error = Some(error.to_string()),
            });
        },
    )
}
