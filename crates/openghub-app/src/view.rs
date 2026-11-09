use rune::tok::{self, radius, space, type_};
use rune::{Color, Cx, El, col, row, text};

#[cfg(unix)]
use crate::app::refresh;
#[cfg(unix)]
use crate::device::{card_tile, device_page};
use crate::state::{Paint, State};
pub(crate) fn view(s: &State, cx: &mut Cx<State>) -> El {
    let dark = cx.system_dark();
    let p = Paint {
        t: tok::theme(dark),
        accent: accent(dark),
    };
    let scale = cx.env().scale;
    #[cfg(unix)]
    if let Some(i) = s.selected {
        return device_page(s, i, p);
    }
    dashboard(s, p, scale)
}

pub(crate) fn accent(_dark: bool) -> Color {
    tok::accent(0, true)
}

pub(crate) fn dashboard(s: &State, p: Paint, scale: f32) -> El {
    let t = p.t;
    col()
        .key("dashboard")
        .w_full()
        .h_full()
        .fill(t.bg)
        .px(space::S6)
        .py(space::S6)
        .child(header_row(t))
        .child(
            col()
                .key("grid")
                .grow(1.0)
                .pt(space::S3)
                .child(cards_grid(s, p, scale)),
        )
}

pub(crate) fn header_row(t: tok::Theme) -> El {
    let row = row()
        .key("header")
        .w_full()
        .center_y()
        .justify_between()
        .child(
            text("Open G Hub")
                .key("title")
                .font(type_::TITLE)
                .color(t.text),
        );
    #[cfg(unix)]
    let row = row.child(refresh_button(t));
    row
}

#[cfg(unix)]
pub(crate) fn refresh_button(t: tok::Theme) -> El {
    row()
        .key("refresh")
        .role("button")
        .label("Refresh")
        .h(28.0)
        .px(space::S3)
        .center()
        .radius(radius::MD)
        .fill(t.card)
        .border(1.0, t.card_border)
        .focusable(true)
        .on_press(|_s: &mut State| refresh())
        .child(text("Refresh").font(type_::LABEL).color(t.text))
}

#[cfg(unix)]
pub(crate) fn cards_grid(s: &State, p: Paint, scale: f32) -> El {
    if s.cards.is_empty() {
        empty_state(p)
    } else {
        let mut r = row()
            .key("cards")
            .gap(space::S3)
            .flex_wrap()
            .justify_center();
        for (i, card) in s.cards.iter().enumerate() {
            r = r.child(card_tile(p, i, card, s.images.as_ref(), scale));
        }
        r
    }
}

#[cfg(not(unix))]
pub(crate) fn cards_grid(_s: &State, p: Paint, _scale: f32) -> El {
    empty_state(p)
}

pub(crate) fn empty_state(p: Paint) -> El {
    let t = p.t;
    col()
        .key("empty")
        .grow(1.0)
        .center()
        .child(
            text("No supported Logitech devices found.\nMake sure the device is plugged in and the udev rule has been applied (see the README).")
                .key("hint")
                .font(type_::BODY)
                .color(t.text_secondary)
                .wrap(),
        )
}
