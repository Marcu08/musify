use rune::{El, ScrollSpec, col, el, row, text, text_input};

use crate::i18n;
use crate::state::{UiState, View};
use crate::theme::{self, palette};
use crate::view::common;

pub fn view(s: &UiState) -> El {
    let p = palette();
    let query = text_input(&s.podcast_query, theme::BODY)
        .key("podcast-search")
        .single_line()
        .w(300.0)
        .h(36.0)
        .fill(p.card)
        .radius(8.0)
        .padding(9.0)
        .color(p.text)
        .placeholder(i18n::PODCAST_SEARCH_PLACEHOLDER, p.text_secondary)
        .on_edit(|s: &mut UiState| &mut s.podcast_query);
    let search = common::action_button(i18n::PODCAST_SEARCH, true)
        .on_press(|s: &mut UiState| s.search_podcasts());
    let subscriptions = common::action_button(i18n::PODCAST_SUBSCRIPTIONS, false)
        .on_press(|s: &mut UiState| s.refresh_podcast_subscriptions());

    let mut heading = col()
        .w_full()
        .gap(4.0)
        .child(
            text(i18n::PODCAST_TITLE)
                .font(theme::PAGE_TITLE)
                .color(p.text),
        )
        .child(
            text(i18n::PODCAST_HINT)
                .font(theme::ROW_META)
                .color(p.text_secondary),
        );
    if s.podcast_loading {
        heading = heading.child(
            text(i18n::PODCAST_LOADING)
                .font(theme::TINY)
                .color(p.accent),
        );
    }

    let podcast_list = if s.podcast_items.is_empty() {
        common::empty_state(i18n::PODCAST_EMPTY, i18n::PODCAST_EMPTY_HINT, None)
    } else {
        let mut rows = col().w_full().gap(2.0);
        for item in &s.podcast_items {
            rows = rows.child(podcast_row(s, item));
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

    let left = col()
        .w(370.0)
        .h_full()
        .min_h(0.0)
        .gap(8.0)
        .child(podcast_list);

    let episodes = if let Some(id) = &s.podcast_selected {
        episodes_panel(s, id)
    } else {
        col().grow(1.0).h_full().center().child(common::empty_state(
            i18n::PODCAST_NO_SELECTION,
            i18n::PODCAST_NO_SELECTION_HINT,
            None,
        ))
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
                .child(query)
                .child(search)
                .child(subscriptions),
        )
        .child(common::divider())
        .child(
            row()
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .gap(16.0)
                .child(left)
                .child(episodes),
        )
        .child(
            row().child(
                common::small_action(format!("< {}", i18n::BACK))
                    .on_press(|s: &mut UiState| s.set_view(View::Sources)),
            ),
        )
        .child(el().h(8.0))
}

fn podcast_row(s: &UiState, item: &musify_core::SourceItem) -> El {
    let p = palette();
    let open_id = item.id.clone();
    let favorite_id = item.id.clone();
    let favorite = item.favorite;
    let subtitle = if item.subtitle.is_empty() {
        item.detail.clone()
    } else {
        item.subtitle.clone()
    };
    row()
        .key(format!("podcast-{}", item.id))
        .w_full()
        .h(66.0)
        .px(10.0)
        .gap(10.0)
        .center_y()
        .radius(theme::RADIUS)
        .hover(p.hover)
        .child(common::cover(
            s,
            item.image_path.as_deref(),
            42.0,
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
                        .color(p.text)
                        .truncate(),
                )
                .child(
                    text(subtitle)
                        .font(theme::ROW_META)
                        .color(p.text_secondary)
                        .truncate(),
                ),
        )
        .child(
            common::small_action(if favorite {
                i18n::PODCAST_UNSUBSCRIBE
            } else {
                i18n::PODCAST_SUBSCRIBE
            })
            .on_press(move |s: &mut UiState| s.toggle_podcast_subscription(&favorite_id)),
        )
        .child(
            common::action_button(i18n::PODCAST_OPEN, true)
                .on_press(move |s: &mut UiState| s.open_podcast(&open_id)),
        )
}

fn episodes_panel(s: &UiState, id: &str) -> El {
    let p = palette();
    let id = id.to_string();
    let mut header = row().w_full().gap(10.0).center_y().child(
        text(s.podcast_selected_title.clone())
            .font(theme::SECTION_TITLE)
            .color(p.text)
            .grow(1.0),
    );
    if s.podcast_entries_loading {
        header = header.child(
            text(i18n::PODCAST_EPISODES_LOADING)
                .font(theme::TINY)
                .color(p.accent),
        );
    }
    header = header.child(
        common::action_button(i18n::PODCAST_REFRESH, false)
            .on_press(move |s: &mut UiState| s.refresh_podcast(&id)),
    );

    let list = if s.podcast_entries.is_empty() {
        common::empty_state(
            i18n::PODCAST_NO_EPISODES,
            i18n::PODCAST_NO_EPISODES_HINT,
            None,
        )
    } else {
        let mut rows = col().w_full().gap(4.0);
        for entry in &s.podcast_entries {
            rows = rows.child(episode_row(s, entry));
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
        .grow(1.0)
        .h_full()
        .min_w(0.0)
        .min_h(0.0)
        .gap(8.0)
        .child(header)
        .child(common::divider())
        .child(list)
}

fn episode_row(s: &UiState, entry: &musify_core::SourceEntry) -> El {
    let p = palette();
    let play_id = entry.id.clone();
    let watched_id = entry.id.clone();
    let watched = entry.watched;
    let resume = entry.position.map(|position| {
        if entry.watched {
            i18n::PODCAST_WATCHED.to_string()
        } else {
            format!("{} {}", i18n::PODCAST_RESUME, i18n::duration_text(position))
        }
    });
    let mut meta = entry.subtitle.clone();
    if let Some(resume) = resume {
        if meta.is_empty() {
            meta = resume;
        } else {
            meta = format!("{meta} · {resume}");
        }
    }
    row()
        .key(format!("episode-{}", entry.id))
        .w_full()
        .min_h(72.0)
        .px(10.0)
        .py(8.0)
        .gap(10.0)
        .radius(theme::RADIUS)
        .hover(p.hover)
        .child(common::cover(
            s,
            entry.image_path.as_deref(),
            44.0,
            8.0,
            &entry.title,
        ))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(3.0)
                .child(
                    text(entry.title.clone())
                        .font(theme::ROW_TITLE)
                        .color(p.text)
                        .truncate(),
                )
                .child(
                    text(meta)
                        .font(theme::ROW_META)
                        .color(p.text_secondary)
                        .truncate(),
                )
                .child(
                    text(entry.detail.clone())
                        .font(theme::TINY)
                        .color(p.text_secondary.alpha(0.8))
                        .truncate(),
                ),
        )
        .child(
            common::small_action(if watched {
                i18n::PODCAST_MARK_UNWATCHED
            } else {
                i18n::PODCAST_MARK_WATCHED
            })
            .on_press(move |s: &mut UiState| {
                s.toggle_podcast_watched(&watched_id, !watched);
            }),
        )
        .child(
            common::action_button(i18n::PODCAST_PLAY, true)
                .on_press(move |s: &mut UiState| s.play_podcast_entry(&play_id)),
        )
        .child(el().w(2.0))
}
