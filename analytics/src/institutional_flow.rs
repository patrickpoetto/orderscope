use std::collections::VecDeque;

use orderscope_core::Side;

/// Tracks the buy/sell mix among recent large ("institutional-sized") trade prints.
///
/// This is a heuristic, not a certainty: a real institutional order is usually
/// algorithmically sliced specifically to avoid looking like one large block, so
/// filtering by raw print size mostly surfaces prints that are visibly large, not
/// necessarily "smart money" — the same limitation every retail footprint tool has.
/// `size_threshold` currently needs a manual per-symbol tuning knob (no free
/// average-daily-volume feed to derive it automatically without extra REST budget).
pub struct InstitutionalFlowTracker {
    large_trades: VecDeque<(f64, Side)>,
    capacity: usize,
    size_threshold: f64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct InstitutionalFlowSnapshot {
    /// Fraction of recent large-print volume that was buy-side, in `[0, 1]`.
    pub buy_ratio: f64,
    pub large_trade_count: u64,
}

impl InstitutionalFlowTracker {
    pub fn new(size_threshold: f64, capacity: usize) -> Self {
        Self { large_trades: VecDeque::with_capacity(capacity), capacity, size_threshold }
    }

    pub fn on_trade(&mut self, qty: f64, side: Side) {
        if qty < self.size_threshold {
            return;
        }
        if self.large_trades.len() == self.capacity {
            self.large_trades.pop_front();
        }
        self.large_trades.push_back((qty, side));
    }

    pub fn snapshot(&self) -> InstitutionalFlowSnapshot {
        if self.large_trades.is_empty() {
            return InstitutionalFlowSnapshot::default();
        }
        let buy_qty: f64 =
            self.large_trades.iter().filter(|(_, side)| *side == Side::Buy).map(|(qty, _)| qty).sum();
        let total_qty: f64 = self.large_trades.iter().map(|(qty, _)| qty).sum();

        InstitutionalFlowSnapshot {
            buy_ratio: if total_qty > 0.0 { buy_qty / total_qty } else { 0.0 },
            large_trade_count: self.large_trades.len() as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_trades_below_the_size_threshold() {
        let mut t = InstitutionalFlowTracker::new(500.0, 20);
        t.on_trade(100.0, Side::Buy);
        assert_eq!(t.snapshot().large_trade_count, 0);
    }

    #[test]
    fn computes_buy_ratio_among_large_prints_only() {
        let mut t = InstitutionalFlowTracker::new(500.0, 20);
        t.on_trade(50.0, Side::Sell); // below threshold, ignored
        t.on_trade(600.0, Side::Buy);
        t.on_trade(400.0, Side::Sell); // below threshold, ignored
        t.on_trade(600.0, Side::Sell);
        let snap = t.snapshot();
        assert_eq!(snap.large_trade_count, 2);
        assert!((snap.buy_ratio - 0.5).abs() < 1e-9);
    }

    #[test]
    fn evicts_oldest_beyond_capacity() {
        let mut t = InstitutionalFlowTracker::new(500.0, 2);
        t.on_trade(600.0, Side::Sell);
        t.on_trade(600.0, Side::Sell);
        t.on_trade(600.0, Side::Buy); // evicts the first Sell
        let snap = t.snapshot();
        assert_eq!(snap.large_trade_count, 2);
        assert!((snap.buy_ratio - 0.5).abs() < 1e-9);
    }
}
