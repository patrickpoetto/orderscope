use std::sync::Arc;

use crate::trade::Trade;
use crate::unit::Symbol;

/// What a given stream is. Only `Trades` exists today because no free source we use
/// provides live depth or pre-made klines. Adding a `Depth` or `Kline` variant later
/// is a small additive change confined to this enum, the adapter that produces it,
/// and the app's event-handling match — unlike the pane-content problem, there is no
/// wide fan-out of match sites for this enum.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StreamKind {
    Trades { symbol: Symbol },
}

/// The normalized event type every data-source adapter funnels into.
#[derive(Debug, Clone)]
pub enum Event {
    Connected(Arc<[StreamKind]>),
    Disconnected(Arc<[StreamKind]>, String),
    TradesReceived(StreamKind, Box<[Trade]>),
}
