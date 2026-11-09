use rune::tok;
use rune::{Color, Image, ImageCache};

#[cfg(unix)]
use crate::device::Card;

#[derive(Default)]
pub(crate) struct State {
    #[cfg(unix)]
    pub(crate) cards: Vec<Card>,
    #[cfg(unix)]
    pub(crate) selected: Option<usize>,
    pub(crate) images: Option<ImageCache>,
    #[cfg(unix)]
    pub(crate) preview_image: Option<Image>,
    #[cfg(unix)]
    pub(crate) preview_pending: Option<Image>,
    #[cfg(unix)]
    pub(crate) preview_seq: u64,
    /// Latest frame clock from `poll`; drives LED preview animation.
    pub(crate) clock: f64,
}

#[derive(Clone, Copy)]
pub(crate) struct Paint {
    pub(crate) t: tok::Theme,
    pub(crate) accent: Color,
}
