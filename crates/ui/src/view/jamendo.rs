use rune::{El, GrowthSpec, Link, ScrollSpec, col, el, row, spacer, text, text_input};

use crate::i18n;
use crate::state::{JamendoSort, UiState};
use crate::theme::{self, palette};
use crate::view::common;

const GENRES_PER_ROW: usize = 6;

pub fn view(s: &UiState) -> El {
    if !s.jamendo_available {
        return common::empty_state(
            i18n::JAMENDO_NOT_CONFIGURED,
            i18n::JAMENDO_CONFIG_HINT,
            None,
        );
    }

    let heading = heading(s);
    let genres = genre_grid(s);
    let sorts = sort_row(s);
    let search = search_row(s);
    let selections = selections_section(s);
    let actions = actions_row(s);
    let list = track_list(s);
    let footer = footer(s);

    col()
        .w_full()
        .h_full()
        .px(26.0)
        .gap(4.0)
        .child(heading)
        .child(genres)
        .child(sorts)
        .child(search)
        .child(selections)
        .child(common::divider())
        .child(actions)
        .child(list)
        .child(footer)
        .child(el().h(4.0))
}

fn heading(s: &UiState) -> El {
    let p = palette();
    let status = if s.jamendo_loading {
        Some(i18n::JAMENDO_LOADING.to_string())
    } else if s.jamendo_loading_more {
        Some(i18n::JAMENDO_LOADING_MORE.to_string())
    } else if s.jamendo_offline {
        Some(i18n::JAMENDO_OFFLINE.to_string())
    } else {
        s.jamendo_latency_ms
            .map(|ms| i18n::jamendo_latency(ms, s.jamendo_latency_cached))
    };
    let mut title = row().w_full().gap(8.0).center_y().child(
        text(i18n::JAMENDO_TITLE)
            .font(theme::PAGE_TITLE)
            .color(p.text),
    );
    if let Some(status) = status {
        title = title
            .child(spacer())
            .child(text(status).font(theme::TINY).color(p.accent));
    }
    col().w_full().gap(2.0).child(title).child(
        text(i18n::JAMENDO_HINT)
            .font(theme::ROW_META)
            .color(p.text_secondary)
            .max_w(720.0)
            .wrap(),
    )
}

fn genre_grid(s: &UiState) -> El {
    let genres = i18n::JAMENDO_GENRES;
    let mut grid = col().w_full().gap(6.0);
    for row_index in 0..genres.len().div_ceil(GENRES_PER_ROW) {
        let mut line = row().w_full().gap(6.0);
        for column in 0..GENRES_PER_ROW {
            let Some((tag, label)) = genres.get(row_index * GENRES_PER_ROW + column) else {
                line = line.child(col().grow(1.0));
                continue;
            };
            let active = s.jamendo_selection.is_none() && s.jamendo_genre == *tag;
            let selected_tag = (*tag).to_string();
            line = line.child(
                common::choice_chip(label, active)
                    .grow(1.0)
                    .on_press(move |s: &mut UiState| s.select_jamendo_genre(&selected_tag)),
            );
        }
        grid = grid.child(line);
    }
    grid
}

fn sort_row(s: &UiState) -> El {
    let p = palette();
    let mut line = row().w_full().gap(6.0).center_y();
    for sort in JamendoSort::ALL {
        let active = s.jamendo_selection.is_none() && s.jamendo_sort == sort;
        line = line.child(
            common::pill(sort.label(), active)
                .h(28.0)
                .on_press(move |s: &mut UiState| s.set_jamendo_sort(sort)),
        );
    }
    line = line.child(spacer());
    let current = if let Some(selection) = &s.jamendo_selection {
        if s.jamendo_selection_name.is_empty() {
            selection.clone()
        } else {
            s.jamendo_selection_name.clone()
        }
    } else {
        i18n::jamendo_genre_label(&s.jamendo_genre)
    };
    line.child(text(current).font(theme::TINY).color(p.text_secondary))
        .child(text("·").font(theme::TINY).color(p.text_secondary))
        .child(
            text(i18n::tracks_count(s.jamendo_items.len()))
                .font(theme::TINY)
                .color(p.text_secondary),
        )
}

fn search_row(s: &UiState) -> El {
    let p = palette();
    let input = text_input(&s.jamendo_query, theme::BODY)
        .key("jamendo-query")
        .single_line()
        .w(300.0)
        .h(34.0)
        .fill(p.card)
        .radius(8.0)
        .padding(9.0)
        .color(p.text)
        .placeholder(i18n::JAMENDO_SEARCH_PLACEHOLDER, p.text_secondary)
        .on_edit(|s: &mut UiState| &mut s.jamendo_query)
        .on_submit(|s: &mut UiState, _tx: &mut rune::Tx| s.search_jamendo());
    row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(input)
        .child(
            common::action_button(i18n::JAMENDO_SEARCH, true)
                .h(30.0)
                .on_press(|s: &mut UiState| s.search_jamendo()),
        )
        .child(
            common::action_button(i18n::JAMENDO_FAVORITES, false)
                .h(30.0)
                .on_press(|s: &mut UiState| s.refresh_jamendo_favorites()),
        )
        .child(spacer())
        .child(
            text(i18n::JAMENDO_KEYBOARD_HINT)
                .font(theme::TINY)
                .color(p.text_secondary.alpha(0.7))
                .max_w(320.0)
                .wrap(),
        )
}

fn selections_section(s: &UiState) -> El {
    let p = palette();
    if s.jamendo_selections.is_empty() {
        if s.jamendo_selections_loading {
            return row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(
                    text(i18n::JAMENDO_SELECTIONS)
                        .font(theme::TINY)
                        .color(p.text_secondary),
                )
                .child(
                    text(i18n::JAMENDO_SELECTIONS_LOADING)
                        .font(theme::TINY)
                        .color(p.accent),
                );
        }
        return el().h(0.0);
    }
    let mut line = row().w_full().gap(5.0).center_y().child(
        text(i18n::JAMENDO_SELECTIONS)
            .font(theme::TINY)
            .color(p.text_secondary),
    );
    for item in &s.jamendo_selections {
        line = line.child(selection_card(s, item));
    }
    line.child(spacer()).child(
        common::small_action(i18n::JAMENDO_REFRESH_SELECTIONS)
            .on_press(|s: &mut UiState| s.refresh_jamendo_selections()),
    )
}

fn selection_card(s: &UiState, item: &musify_core::SourceItem) -> El {
    let p = palette();
    let active = s.jamendo_selection.as_deref() == Some(item.id.as_str());
    let id = item.id.clone();
    row()
        .key(format!("jamendo-selection-{}", item.id))
        .h(26.0)
        .px(7.0)
        .gap(5.0)
        .center_y()
        .radius(theme::RADIUS)
        .fill(if active { p.accent_soft } else { p.card })
        .hover(p.hover)
        .on_press(move |s: &mut UiState| s.open_jamendo_selection(&id))
        .child(common::cover(
            s,
            item.image_path.as_deref(),
            18.0,
            4.0,
            &item.title,
        ))
        .child(
            text(item.title.clone())
                .font(theme::BUTTON)
                .color(if active { p.accent } else { p.text })
                .max_w(130.0)
                .truncate(),
        )
}

fn actions_row(s: &UiState) -> El {
    let p = palette();
    let mut line = row().w_full().gap(7.0).center_y();
    if s.jamendo_items.is_empty() {
        line = line.child(
            text(i18n::JAMENDO_EMPTY)
                .font(theme::ROW_META)
                .color(p.text_secondary),
        );
    } else {
        line = line
            .child(
                common::action_button(i18n::PLAY_ALL, true)
                    .h(30.0)
                    .on_press(|s: &mut UiState| s.play_jamendo_list(false)),
            )
            .child(
                common::action_button(i18n::JAMENDO_SHUFFLE, false)
                    .h(30.0)
                    .on_press(|s: &mut UiState| s.play_jamendo_list(true)),
            )
            .child(
                common::action_button(i18n::JAMENDO_ENQUEUE_ALL, false)
                    .h(30.0)
                    .on_press(|s: &mut UiState| s.enqueue_jamendo_list()),
            )
            .child(
                common::action_button(i18n::JAMENDO_SAVE_PLAYLIST, false)
                    .h(30.0)
                    .on_press(|s: &mut UiState| s.save_jamendo_playlist()),
            );
        if s.jamendo_selection.is_some() {
            line = line.child(
                common::small_action(i18n::JAMENDO_BACK_TO_GENRES)
                    .on_press(|s: &mut UiState| s.back_to_jamendo_genres()),
            );
        }
    }
    if let Some(status) = &s.jamendo_action_status {
        line = line.child(spacer()).child(
            text(status.clone())
                .font(theme::TINY)
                .color(p.text_secondary),
        );
    }
    line
}

fn track_list(s: &UiState) -> El {
    if s.jamendo_items.is_empty() {
        if s.jamendo_loading {
            return common::empty_state(i18n::JAMENDO_LOADING, i18n::JAMENDO_EMPTY_HINT, None);
        }
        return common::empty_state(i18n::JAMENDO_EMPTY, i18n::JAMENDO_EMPTY_HINT, None);
    }
    let stack = col()
        .key("jamendo-rows")
        .w_full()
        .growth(GrowthSpec::default())
        .virtual_rows(
            s.jamendo_items.len(),
            76.0,
            |s: &UiState, index| s.jamendo_keys[index].as_str().into(),
            |s: &UiState, index| jamendo_row(s, index),
        )
        .virtual_rows_version(s.list_version);
    col()
        .key("jamendo-list")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(stack)
}

fn footer(s: &UiState) -> El {
    let p = palette();
    if s.jamendo_loading_more {
        return row().w_full().center().child(
            text(i18n::JAMENDO_LOADING_MORE)
                .font(theme::TINY)
                .color(p.accent),
        );
    }
    if s.jamendo_has_more {
        return row().w_full().center().child(
            common::action_button(i18n::JAMENDO_LOAD_MORE, false)
                .on_press(|s: &mut UiState| s.load_more_jamendo()),
        );
    }
    el().h(0.0)
}

fn jamendo_row(s: &UiState, index: usize) -> El {
    let p = palette();
    let Some(item) = s.jamendo_items.get(index) else {
        return el().h(76.0);
    };
    let favorite = item.favorite;
    let favorite_id = item.id.clone();
    let play_id = item.id.clone();
    let double_play_id = item.id.clone();
    let add_id = item.id.clone();
    let selected = s.jamendo_selected == Some(index);
    let duration = item
        .duration
        .map(i18n::duration_text)
        .unwrap_or_else(|| "--:--".to_string());

    let mut detail = row().gap(6.0).center_y();
    if let Some(url) = item
        .license_url
        .clone()
        .filter(|url| !url.trim().is_empty())
    {
        let label = item.detail.clone();
        detail = detail.child(
            text(label.clone())
                .font(theme::TINY)
                .color(p.accent)
                .on_link(|_: &mut UiState, url: &str, tx: &mut rune::Tx| {
                    let _ = tx.open_url(url);
                })
                .link(Link::new(0..label.len(), url)),
        );
    } else if !item.detail.is_empty() {
        detail = detail.child(
            text(item.detail.clone())
                .font(theme::TINY)
                .color(p.text_secondary),
        );
    }
    let page_label = i18n::JAMENDO_TRACK_PAGE.to_string();
    detail = detail
        .child(text("·").font(theme::TINY).color(p.text_secondary))
        .child(
            text(page_label.clone())
                .font(theme::TINY)
                .color(p.accent)
                .on_link(|_: &mut UiState, url: &str, tx: &mut rune::Tx| {
                    let _ = tx.open_url(url);
                })
                .link(Link::new(
                    0..page_label.len(),
                    format!("https://www.jamendo.com/track/{}", item.id),
                )),
        );

    row()
        .key(format!("jamendo-{}", item.id))
        .w_full()
        .h(76.0)
        .px(10.0)
        .gap(8.0)
        .center_y()
        .radius(theme::RADIUS)
        .fill(if selected {
            p.accent_soft
        } else {
            theme::TRANSPARENT
        })
        .hover(p.hover)
        .on_press(move |s: &mut UiState| s.select_jamendo_row(index))
        .on_double_press(move |s: &mut UiState| s.play_jamendo(&double_play_id))
        .child(common::cover(
            s,
            item.image_path.as_deref(),
            40.0,
            8.0,
            &item.title,
        ))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(2.0)
                .child(
                    text(item.title.clone())
                        .font(theme::ROW_TITLE)
                        .color(p.text),
                )
                .child(
                    text(item.subtitle.clone())
                        .font(theme::ROW_META)
                        .color(p.text_secondary),
                )
                .child(detail),
        )
        .child(
            text(duration)
                .font(theme::TIME)
                .color(p.text_secondary)
                .w(44.0),
        )
        .child(
            common::small_action(if favorite {
                i18n::RADIO_REMOVE_FAVORITE
            } else {
                i18n::RADIO_ADD_FAVORITE
            })
            .on_press(move |s: &mut UiState| s.toggle_jamendo_favorite(&favorite_id)),
        )
        .child(
            common::action_button(i18n::JAMENDO_PLAY, true)
                .on_press(move |s: &mut UiState| s.play_jamendo(&play_id)),
        )
        .child(
            common::small_action(i18n::PLAYLIST_ADD)
                .on_press(move |s: &mut UiState| s.add_jamendo_to_selected_playlist(&add_id)),
        )
}
