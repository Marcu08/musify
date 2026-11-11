mod albums;
mod artists;
mod common;
mod content;
mod player;
mod podcast;
mod queue;
mod radio;
mod search;
mod settings;
mod sidebar;
mod sources;
mod topbar;
mod tracks;

use rune::{Cx, El, col, row};

use crate::state::UiState;
use crate::theme::palette;

pub fn view(s: &UiState, cx: &mut Cx<UiState>) -> El {
    s.set_cover_scale(cx.env().scale);
    let p = palette();
    let main = col()
        .grow(1.0)
        .h_full()
        .min_w(0.0)
        .child(topbar::view(s))
        .child(col().w_full().grow(1.0).min_h(0.0).child(content::view(s)))
        .child(player::view(s));
    row()
        .w_full()
        .h_full()
        .fill(p.bg)
        .child(sidebar::view(s))
        .child(main)
}
