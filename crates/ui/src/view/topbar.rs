use rune::{El, col, row, text, text_input};

use crate::i18n;
use crate::state::UiState;
use crate::theme::{self, palette};
use crate::view::common;

pub fn view(s: &UiState) -> El {
    let p = palette();
    let field = text_input(&s.query, theme::BODY)
        .key("search")
        .single_line()
        .w(460.0)
        .h(38.0)
        .fill(p.card)
        .radius(8.0)
        .padding(10.0)
        .color(p.text)
        .placeholder(i18n::SEARCH_PLACEHOLDER, p.text_secondary)
        .on_edit(|s: &mut UiState| &mut s.query)
        .on_input_change(|s: &mut UiState, _| s.refresh_search());

    let rescan = common::action_button(i18n::RESCAN, false).on_press(|s: &mut UiState| s.rescan());

    let status = if let Some(scan) = s.scan {
        i18n::scan_progress(scan.scanned, scan.total)
    } else {
        i18n::tracks_count(s.stats.tracks as usize)
    };

    col()
        .w_full()
        .child(
            row()
                .h(theme::TOPBAR_H)
                .w_full()
                .px(26.0)
                .gap(14.0)
                .center_y()
                .fill(p.bg)
                .child(field)
                .child(rune::spacer())
                .child(text(status).font(theme::SMALL).color(p.text_secondary))
                .child(rescan),
        )
        .child(common::divider())
        .child(col().w_full().h(10.0))
}
