use smallvec::SmallVec;

use crate::unit::{Price, Qty, UnixMs};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

/// Raw SIP trade condition codes, as delivered by the data source (Finnhub's `c` field).
///
/// Kept as opaque numeric codes rather than pre-interpreted flags (e.g. "is dark pool"):
/// the numeric-code-to-meaning mapping is vendor-specific and we don't yet have a
/// verified interpretation for this source (see analytics::dark_pool doc comment).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Conditions(SmallVec<[u16; 4]>);

impl Conditions {
    pub fn from_codes(codes: impl IntoIterator<Item = u16>) -> Self {
        Self(codes.into_iter().collect())
    }

    pub fn contains(&self, code: u16) -> bool {
        self.0.contains(&code)
    }

    pub fn codes(&self) -> &[u16] {
        &self.0
    }
}

/// A single normalized trade print, source-agnostic.
///
/// `side` is intentionally `Option`: most free-tier trade feeds (including Finnhub's)
/// carry no aggressor-side field, so it starts `None` and gets resolved once by
/// `orderscope-analytics::TradeClassifier` before the trade reaches the chart data
/// model. A future source that does provide a real side can populate it directly,
/// letting that trade skip classification with no type change required.
#[derive(Debug, Clone, PartialEq)]
pub struct Trade {
    pub time: UnixMs,
    pub price: Price,
    pub qty: Qty,
    pub side: Option<Side>,
    pub conditions: Conditions,
}
