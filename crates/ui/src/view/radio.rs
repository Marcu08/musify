use rune::{El, ScrollSpec, col, el, row, text, text_input};

use crate::i18n;
use crate::state::{UiState, View};
use crate::theme::{self, palette};
use crate::view::common;

pub fn view(s: &UiState) -> El {
    let p = palette();
    let name = field(&s.radio_query.name, i18n::RADIO_NAME, "radio-name", |s| {
        &mut s.radio_query.name
    });
    let country = field(
        &s.radio_query.country,
        i18n::RADIO_COUNTRY,
        "radio-country",
        |s| &mut s.radio_query.country,
    );
    let tag = field(&s.radio_query.tag, i18n::RADIO_TAG, "radio-tag", |s| {
        &mut s.radio_query.tag
    });
    let search = common::action_button(i18n::RADIO_SEARCH, true)
        .on_press(|s: &mut UiState| s.search_radio());
    let favorites = common::action_button(i18n::RADIO_FAVORITES, false)
        .on_press(|s: &mut UiState| s.refresh_source_favorites("radio"));

    let status = if s.source_loading {
        Some(i18n::RADIO_LOADING.to_string())
    } else if s.source_offline {
        Some(i18n::RADIO_OFFLINE.to_string())
    } else {
        None
    };
    let mut heading = col()
        .w_full()
        .gap(4.0)
        .child(
            text(i18n::RADIO_TITLE)
                .font(theme::PAGE_TITLE)
                .color(p.text),
        )
        .child(
            text(i18n::RADIO_HINT)
                .font(theme::ROW_META)
                .color(p.text_secondary),
        );
    if let Some(status) = status {
        heading = heading.child(text(status).font(theme::TINY).color(p.accent));
    }

    let list = if s.source_items.is_empty() {
        common::empty_state(i18n::RADIO_EMPTY, i18n::RADIO_EMPTY_HINT, None)
    } else {
        let mut rows = col().w_full().gap(2.0);
        for item in &s.source_items {
            rows = rows.child(station_row(s, item));
        }
        col()
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .child(rows)
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
                .child(name)
                .child(country)
                .child(tag)
                .child(search)
                .child(favorites),
        )
        .child(common::divider())
        .child(list)
        .child(
            row().child(
                common::small_action(format!("< {}", i18n::BACK))
                    .on_press(|s: &mut UiState| s.set_view(View::Sources)),
            ),
        )
        .child(el().h(8.0))
}

fn field(
    edit: &rune::TextEdit,
    placeholder: &'static str,
    key: &'static str,
    accessor: fn(&mut UiState) -> &mut rune::TextEdit,
) -> El {
    let p = palette();
    text_input(edit, theme::BODY)
        .key(key)
        .single_line()
        .w(190.0)
        .h(36.0)
        .fill(p.card)
        .radius(8.0)
        .padding(9.0)
        .color(p.text)
        .placeholder(placeholder, p.text_secondary)
        .on_edit(accessor)
}

fn station_row(s: &UiState, item: &musify_core::SourceItem) -> El {
    let p = palette();
    let id = item.id.clone();
    let favorite_id = item.id.clone();
    let favorite = item.favorite;
    let icon = item.image_path.clone();
    let subtitle = if item.detail.is_empty() {
        item.subtitle.clone()
    } else {
        format!("{} · {}", item.subtitle, item.detail)
    };
    row()
        .key(format!("radio-{}", item.id))
        .w_full()
        .h(64.0)
        .px(10.0)
        .gap(12.0)
        .center_y()
        .radius(theme::RADIUS)
        .hover(p.hover)
        .child(common::cover(s, icon.as_deref(), 40.0, 8.0, &item.title))
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
                .child(text(subtitle).font(theme::ROW_META).color(p.text_secondary)),
        )
        .child(
            common::small_action(if favorite {
                i18n::RADIO_REMOVE_FAVORITE
            } else {
                i18n::RADIO_ADD_FAVORITE
            })
            .on_press(move |s: &mut UiState| {
                s.toggle_source_favorite("radio", &favorite_id);
            }),
        )
        .child(
            common::action_button(i18n::RADIO_PLAY, true)
                .on_press(move |s: &mut UiState| s.play_source("radio", &id)),
        )
        .child(el().w(4.0))
        .child(
            text(i18n::source_label(item.source.scheme()))
                .font(theme::TINY)
                .color(p.text_secondary),
        )
}
