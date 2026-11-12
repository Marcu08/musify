use rune::{El, GrowthSpec, Link, ScrollSpec, col, el, row, text, text_input};

use crate::i18n;
use crate::state::UiState;
use crate::theme::{self, palette};
use crate::view::common;

pub fn view(s: &UiState) -> El {
    let p = palette();
    if !s.jamendo_available {
        return common::empty_state(
            i18n::JAMENDO_NOT_CONFIGURED,
            i18n::JAMENDO_CONFIG_HINT,
            None,
        );
    }

    let search = text_input(&s.jamendo_query, theme::BODY)
        .key("jamendo-query")
        .single_line()
        .w(230.0)
        .h(36.0)
        .fill(p.card)
        .radius(8.0)
        .padding(9.0)
        .color(p.text)
        .placeholder(i18n::JAMENDO_SEARCH_PLACEHOLDER, p.text_secondary)
        .on_edit(|s: &mut UiState| &mut s.jamendo_query);
    let tag = text_input(&s.jamendo_tag, theme::BODY)
        .key("jamendo-tag")
        .single_line()
        .w(190.0)
        .h(36.0)
        .fill(p.card)
        .radius(8.0)
        .padding(9.0)
        .color(p.text)
        .placeholder(i18n::JAMENDO_TAG_PLACEHOLDER, p.text_secondary)
        .on_edit(|s: &mut UiState| &mut s.jamendo_tag);
    let search_button = common::action_button(i18n::JAMENDO_SEARCH, true)
        .on_press(|s: &mut UiState| s.search_jamendo());
    let favorites = common::action_button(i18n::JAMENDO_FAVORITES, false)
        .on_press(|s: &mut UiState| s.refresh_jamendo_favorites());

    let status = if s.jamendo_loading {
        Some(i18n::JAMENDO_LOADING.to_string())
    } else if s.jamendo_offline {
        Some(i18n::JAMENDO_OFFLINE.to_string())
    } else {
        None
    };
    let mut heading = col()
        .w_full()
        .gap(4.0)
        .child(
            text(i18n::JAMENDO_TITLE)
                .font(theme::PAGE_TITLE)
                .color(p.text),
        )
        .child(
            text(i18n::JAMENDO_HINT)
                .font(theme::ROW_META)
                .color(p.text_secondary),
        );
    if let Some(status) = status {
        heading = heading.child(text(status).font(theme::TINY).color(p.accent));
    }

    let list = if s.jamendo_items.is_empty() {
        common::empty_state(i18n::JAMENDO_EMPTY, i18n::JAMENDO_EMPTY_HINT, None)
    } else {
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
    };

    col()
        .w_full()
        .h_full()
        .px(26.0)
        .gap(12.0)
        .child(heading)
        .child(
            row()
                .w_full()
                .gap(10.0)
                .center_y()
                .child(search)
                .child(tag)
                .child(search_button)
                .child(favorites),
        )
        .child(common::divider())
        .child(list)
        .child(el().h(8.0))
}

fn jamendo_row(s: &UiState, index: usize) -> El {
    let p = palette();
    let Some(item) = s.jamendo_items.get(index) else {
        return el().h(76.0);
    };
    let favorite = item.favorite;
    let favorite_id = item.id.clone();
    let play_id = item.id.clone();
    let add_id = item.id.clone();
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
        .hover(p.hover)
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
