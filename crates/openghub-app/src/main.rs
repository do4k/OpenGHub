//! Open G Hub — a Logitech G HUB alternative. Rune UI, Rust port of the
//! Avalonia `OpenGHub.App`. Talks to hardware through `openghub-core`.

mod app;
#[cfg(unix)]
mod debounce;
#[cfg(unix)]
mod device;
mod state;
mod view;
#[cfg(unix)]
mod widgets;

fn main() {
    app::run();
}
