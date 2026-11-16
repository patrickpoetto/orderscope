use orderscope_core::{Price, Side};

/// Tick-rule aggressor-side classifier: upticks are buys, downticks are sells,
/// and a zero-tick (repeated price) repeats the previous classification ("sticky").
///
/// This is the only classification method available for Finnhub's free trade feed,
/// which carries no NBBO (so the quote rule isn't usable) and no aggressor-side field.
/// One instance is owned per watched symbol.
pub struct TradeClassifier {
    last_price: Option<Price>,
    last_side: Side,
}

impl Default for TradeClassifier {
    fn default() -> Self {
        Self { last_price: None, last_side: Side::Buy }
    }
}

impl TradeClassifier {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn classify(&mut self, price: Price) -> Side {
        let side = match self.last_price {
            None => Side::Buy, // first trade of the session: documented default
            Some(last) if price > last => Side::Buy,
            Some(last) if price < last => Side::Sell,
            _ => self.last_side, // zero-tick: repeat last classification
        };
        self.last_price = Some(price);
        self.last_side = side;
        side
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_trade_defaults_to_buy() {
        let mut c = TradeClassifier::new();
        assert_eq!(c.classify(Price::from_f64(100.0)), Side::Buy);
    }

    #[test]
    fn uptick_is_buy_downtick_is_sell() {
        let mut c = TradeClassifier::new();
        c.classify(Price::from_f64(100.0));
        assert_eq!(c.classify(Price::from_f64(100.5)), Side::Buy);
        assert_eq!(c.classify(Price::from_f64(100.0)), Side::Sell);
    }

    #[test]
    fn zero_tick_repeats_last_side() {
        let mut c = TradeClassifier::new();
        c.classify(Price::from_f64(100.0)); // Buy (first trade)
        c.classify(Price::from_f64(99.0)); // Sell (downtick)
        assert_eq!(c.classify(Price::from_f64(99.0)), Side::Sell); // zero-tick, sticky
        assert_eq!(c.classify(Price::from_f64(99.0)), Side::Sell);
        assert_eq!(c.classify(Price::from_f64(99.5)), Side::Buy); // uptick flips it
        assert_eq!(c.classify(Price::from_f64(99.5)), Side::Buy); // zero-tick, sticky again
    }
}
