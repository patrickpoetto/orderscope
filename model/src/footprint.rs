use std::collections::HashMap;

use orderscope_core::{Price, PriceStep, Side};

use crate::timeseries::{ClassifiedTrade, DataPoint};

/// Buy/sell volume accumulated at a single price level within one bar.
#[derive(Debug, Clone, Copy, Default)]
pub struct GroupedTrades {
    pub buy_qty: f64,
    pub sell_qty: f64,
    pub buy_count: u32,
    pub sell_count: u32,
}

impl GroupedTrades {
    pub fn delta(&self) -> f64 {
        self.buy_qty - self.sell_qty
    }

    pub fn total(&self) -> f64 {
        self.buy_qty + self.sell_qty
    }
}

/// Bar-level volume, aggregated for cheap redraws (vs. iterating `levels` every frame).
#[derive(Debug, Clone, Copy)]
pub enum Volume {
    BuySell(f64, f64),
}

impl Volume {
    pub fn delta(&self) -> f64 {
        let Volume::BuySell(buy, sell) = self;
        buy - sell
    }

    pub fn total(&self) -> f64 {
        let Volume::BuySell(buy, sell) = self;
        buy + sell
    }
}

impl Default for Volume {
    fn default() -> Self {
        Volume::BuySell(0.0, 0.0)
    }
}

/// One footprint bar: OHLC plus a per-price-level buy/sell cluster map and its
/// point of control (the price level with the most total volume in the bar).
#[derive(Debug, Clone, Default)]
pub struct FootprintBar {
    pub open: Option<Price>,
    pub high: Option<Price>,
    pub low: Option<Price>,
    pub close: Option<Price>,
    pub volume: Volume,
    pub levels: HashMap<Price, GroupedTrades>,
    pub poc: Option<Price>,
}

impl DataPoint for FootprintBar {
    fn add_trade(&mut self, trade: &ClassifiedTrade, tick: PriceStep) {
        if self.open.is_none() {
            self.open = Some(trade.price);
        }
        self.high = Some(self.high.map_or(trade.price, |h| h.max(trade.price)));
        self.low = Some(self.low.map_or(trade.price, |l| l.min(trade.price)));
        self.close = Some(trade.price);

        let bucket = trade.price.round_to_step(tick);
        let level = self.levels.entry(bucket).or_default();
        match trade.side {
            Side::Buy => {
                level.buy_qty += trade.qty.0;
                level.buy_count += 1;
            }
            Side::Sell => {
                level.sell_qty += trade.qty.0;
                level.sell_count += 1;
            }
        }

        let Volume::BuySell(buy, sell) = self.volume;
        self.volume = match trade.side {
            Side::Buy => Volume::BuySell(buy + trade.qty.0, sell),
            Side::Sell => Volume::BuySell(buy, sell + trade.qty.0),
        };

        self.recompute_poc();
    }
}

impl FootprintBar {
    fn recompute_poc(&mut self) {
        self.poc = self
            .levels
            .iter()
            .max_by(|a, b| a.1.total().total_cmp(&b.1.total()))
            .map(|(price, _)| *price);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orderscope_core::{Qty, UnixMs};

    fn trade(price: f64, qty: f64, side: Side) -> ClassifiedTrade {
        ClassifiedTrade { time: UnixMs(0), price: Price::from_f64(price), qty: Qty(qty), side }
    }

    #[test]
    fn tracks_ohlc_and_delta() {
        let tick = PriceStep::from_f64(0.01);
        let mut bar = FootprintBar::default();
        bar.add_trade(&trade(100.0, 10.0, Side::Buy), tick);
        bar.add_trade(&trade(101.0, 4.0, Side::Sell), tick);
        bar.add_trade(&trade(99.5, 6.0, Side::Buy), tick);

        assert_eq!(bar.open, Some(Price::from_f64(100.0)));
        assert_eq!(bar.close, Some(Price::from_f64(99.5)));
        assert_eq!(bar.high, Some(Price::from_f64(101.0)));
        assert_eq!(bar.low, Some(Price::from_f64(99.5)));
        assert_eq!(bar.volume.delta(), 10.0 + 6.0 - 4.0);
    }

    #[test]
    fn poc_is_the_price_level_with_most_total_volume() {
        let tick = PriceStep::from_f64(1.0); // coarse bucket so 100.1/100.4 land together
        let mut bar = FootprintBar::default();
        bar.add_trade(&trade(100.1, 50.0, Side::Buy), tick);
        bar.add_trade(&trade(100.4, 20.0, Side::Sell), tick);
        bar.add_trade(&trade(105.0, 1.0, Side::Buy), tick);

        assert_eq!(bar.poc, Some(Price::from_f64(100.0)));
    }
}
