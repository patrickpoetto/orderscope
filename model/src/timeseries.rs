use std::collections::BTreeMap;

use orderscope_core::{Price, PriceStep, Qty, Side, UnixMs};

/// A trade with its aggressor side already resolved (by
/// `orderscope_analytics::TradeClassifier`, once, before it reaches the bucketing layer).
#[derive(Debug, Clone, Copy)]
pub struct ClassifiedTrade {
    pub time: UnixMs,
    pub price: Price,
    pub qty: Qty,
    pub side: Side,
}

/// Something that can accumulate classified trades into one time bucket (a bar).
pub trait DataPoint: Default {
    fn add_trade(&mut self, trade: &ClassifiedTrade, tick: PriceStep);
}

/// Client-side bucketing of a trade stream into fixed-width time bars.
///
/// Finnhub (like most free trade-only feeds) pushes no pre-made klines, so bars are
/// built here directly from trades by flooring each trade's timestamp to the interval.
pub struct TimeSeries<D: DataPoint> {
    pub buckets: BTreeMap<UnixMs, D>,
    interval_ms: u64,
    tick: PriceStep,
}

impl<D: DataPoint> TimeSeries<D> {
    pub fn new(interval_ms: u64, tick: PriceStep) -> Self {
        Self { buckets: BTreeMap::new(), interval_ms, tick }
    }

    pub fn ingest(&mut self, trade: &ClassifiedTrade) {
        let floored = trade.time.floor_to(self.interval_ms);
        self.buckets.entry(floored).or_default().add_trade(trade, self.tick);
    }

    /// The most recent `n` bars, oldest first.
    pub fn last_n(&self, n: usize) -> Vec<(UnixMs, &D)> {
        let mut out: Vec<_> = self.buckets.iter().rev().take(n).map(|(t, d)| (*t, d)).collect();
        out.reverse();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct SumBar {
        total: f64,
    }
    impl DataPoint for SumBar {
        fn add_trade(&mut self, trade: &ClassifiedTrade, _tick: PriceStep) {
            self.total += trade.qty.0;
        }
    }

    #[test]
    fn floors_trades_into_the_right_bucket() {
        let mut series: TimeSeries<SumBar> = TimeSeries::new(60_000, PriceStep::from_f64(0.01));
        let mk = |ms: u64, qty: f64| ClassifiedTrade {
            time: UnixMs(ms),
            price: Price::from_f64(100.0),
            qty: Qty(qty),
            side: Side::Buy,
        };

        series.ingest(&mk(1_000, 10.0)); // bucket 0
        series.ingest(&mk(59_999, 5.0)); // bucket 0
        series.ingest(&mk(60_000, 7.0)); // bucket 60_000

        assert_eq!(series.buckets.len(), 2);
        assert_eq!(series.buckets[&UnixMs(0)].total, 15.0);
        assert_eq!(series.buckets[&UnixMs(60_000)].total, 7.0);
    }
}
