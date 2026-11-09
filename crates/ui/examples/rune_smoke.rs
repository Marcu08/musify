//! M0 Rune smoke test, kept in the repository so the pinned rev can be re-checked.
//!
//! ```sh
//! cargo run -p musify-ui --example rune_smoke -- --snap smoke.png
//! cargo run -p musify-ui --example rune_smoke -- --script script.json --snap smoke.png --at mark:end
//! ```

use rune::tok::{self, type_};
use rune::{AppSpec, Color, Cx, El, GrowthSpec, ScrollSpec, TextEdit, col, row, text, text_input};

const ROWS: usize = 100_000;
const ROW_HEIGHT: f32 = 32.0;

struct State {
    query: TextEdit,
    last_query: String,
    keys: Vec<String>,
    clicks: u32,
}

impl Default for State {
    fn default() -> Self {
        let mut query = TextEdit::new("");
        query.focused = true;
        Self {
            query,
            last_query: String::new(),
            keys: (0..ROWS).map(|i| format!("track-{i}")).collect(),
            clicks: 0,
        }
    }
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    let t = tok::theme(true);
    let accent = tok::accent(0, true);

    let field = text_input(&s.query, type_::BODY)
        .key("search")
        .single_line()
        .grow(1.0)
        .color(t.text)
        .placeholder("Cerca nella libreria", t.text_secondary)
        .on_edit(|s: &mut State| &mut s.query);

    let play = row()
        .key("play")
        .h(40.0)
        .px(16.0)
        .radius(8.0)
        .center_y()
        .fill(accent)
        .child(text("Riproduci").font(type_::BODY).color(Color::WHITE))
        .on_press(|s: &mut State| s.clicks += 1);

    let bottom = row()
        .key("bottom")
        .h(40.0)
        .px(14.0)
        .radius(8.0)
        .center_y()
        .fill(t.card)
        .child(text("Vai in fondo").font(type_::BODY).color(t.text))
        .on_press_tx(|_: &mut State, tx| {
            tx.scroll_to_instant("rows", 0.0, ROWS as f32 * ROW_HEIGHT);
        });

    let row_text = t.text;
    let row_secondary = t.text_secondary;
    let rows = col()
        .key("rows")
        .w_full()
        .growth(GrowthSpec::default())
        .virtual_rows(
            ROWS,
            ROW_HEIGHT,
            |s: &State, i| s.keys[i].as_str().into(),
            move |_: &State, i| {
                row()
                    .w_full()
                    .h(ROW_HEIGHT)
                    .px(12.0)
                    .gap(12.0)
                    .center_y()
                    .child(
                        text(format!("#{i:06}"))
                            .font(type_::LABEL)
                            .color(row_secondary),
                    )
                    .child(
                        text(format!("Brano fittizio {}", i + 1))
                            .font(type_::BODY)
                            .color(row_text),
                    )
            },
        )
        .virtual_rows_version(0u32);

    let list = col()
        .key("list")
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(rows);

    col()
        .w_full()
        .h_full()
        .fill(t.bg)
        .padding(16.0)
        .gap(12.0)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .child(field)
                .child(play)
                .child(bottom),
        )
        .child(
            text(format!(
                "{ROWS} righe virtuali · click sul pulsante: {}",
                s.clicks
            ))
            .font(type_::LABEL)
            .color(t.text_secondary),
        )
        .child(list)
}

fn main() {
    let mut spec = AppSpec::new("Musify smoke test", (960.0, 640.0), State::default, view);
    spec.scenes = &[("typed", |s| {
        s.query = TextEdit::new("città 日本語");
        s.query.focused = true;
        s.clicks = 3;
    })];
    spec.poll = Some(|s: &mut State, _t, _tx| {
        if s.query.text != s.last_query {
            s.last_query.clone_from(&s.query.text);
            println!("QUERY={}", s.last_query);
        }
        false
    });
    rune::run(spec);
}
