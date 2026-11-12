use rune::{El, col, el, row, text};

use crate::i18n;
use crate::state::{UiState, View};
use crate::theme::{self, palette};
use crate::view::common;

pub fn view(s: &UiState) -> El {
    let p = palette();
    let local_caption = i18n::tracks_count(s.tracks.len());
    let local = source_card(
        i18n::NAV_LOCAL,
        &local_caption,
        i18n::SOURCE_OPEN,
        View::Tracks,
    );
    let radio = source_card(
        i18n::NAV_RADIO,
        i18n::SOURCES_RADIO_CAPTION,
        i18n::SOURCE_OPEN,
        View::Radio,
    );
    let podcast = source_card(
        i18n::NAV_PODCAST,
        i18n::SOURCES_PODCAST_CAPTION,
        i18n::SOURCE_OPEN,
        View::Podcast,
    );
    let cards = if s.jamendo_available {
        let jamendo = source_card(
            i18n::JAMENDO_TITLE,
            i18n::SOURCES_JAMENDO_CAPTION,
            i18n::SOURCE_OPEN,
            View::Jamendo,
        );
        col()
            .w_full()
            .gap(14.0)
            .child(row().w_full().gap(14.0).child(local).child(radio))
            .child(row().w_full().gap(14.0).child(podcast).child(jamendo))
    } else {
        row()
            .w_full()
            .gap(14.0)
            .child(local)
            .child(radio)
            .child(podcast)
    };
    let mut content = col()
        .w_full()
        .h_full()
        .px(26.0)
        .gap(12.0)
        .child(
            text(i18n::SOURCES_TITLE)
                .font(theme::PAGE_TITLE)
                .color(p.text),
        )
        .child(
            text(i18n::SOURCES_HINT)
                .font(theme::ROW_META)
                .color(p.text_secondary),
        )
        .child(cards);
    if !s.jamendo_available {
        content = content.child(
            col()
                .w_full()
                .gap(4.0)
                .padding(14.0)
                .radius(theme::RADIUS)
                .fill(p.card)
                .child(
                    text(i18n::JAMENDO_NOT_CONFIGURED)
                        .font(theme::SECTION_TITLE)
                        .color(p.text),
                )
                .child(
                    text(i18n::JAMENDO_CONFIG_HINT)
                        .font(theme::ROW_META)
                        .color(p.text_secondary),
                ),
        );
    }
    content.child(el().h(12.0))
}

fn source_card(title: &'static str, caption: &str, action: &'static str, view: View) -> El {
    let p = palette();
    col()
        .grow(1.0)
        .min_w(0.0)
        .gap(8.0)
        .padding(18.0)
        .radius(theme::RADIUS_LG)
        .fill(p.card)
        .child(text(title).font(theme::SECTION_TITLE).color(p.text))
        .child(
            text(caption.to_string())
                .font(theme::ROW_META)
                .color(p.text_secondary)
                .max_w(300.0),
        )
        .child(row().w_full().child(rune::spacer()).child(
            common::action_button(action, true).on_press(move |s: &mut UiState| s.set_view(view)),
        ))
}
