//! M8: pane content as a trait, not a closed enum.
//!
//! Flowsurface's dashboard uses a closed `Content` enum matched at ~150 sites
//! across view/update/subscription/serialization. We know upfront we want several
//! non-chart pane kinds over time (this insider/13F panel now, more from the OSINT
//! data-source track later), so each pane kind lives in its own module implementing
//! this trait; adding a new one means one new `impl` plus one new arm in the single
//! small dispatcher in `main.rs` — not touching logic scattered across the app.

pub mod footprint_cvd;
pub mod insider;

use iced::Element;

use crate::Message;

/// `view` consumes `self` (rather than borrowing) so implementors own their data
/// outright — required for a freshly-built `Box<dyn PaneContent>` local to a
/// `pane_grid` view closure to produce an `Element` that outlives that closure.
pub trait PaneContent {
    fn title(&self) -> String;
    fn view(self: Box<Self>) -> Element<'static, Message>;
}

/// Lightweight, `Copy` marker stored in `pane_grid::State` — just enough to know
/// which `PaneContent` to build for a given grid cell. The actual pane data lives
/// in `App` and is borrowed fresh into a `PaneContent` implementor on every `view()`
/// call, the same pattern already used for `FootprintChart`/`CvdChart`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneKind {
    FootprintCvd,
    Insider,
}
