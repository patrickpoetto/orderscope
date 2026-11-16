use std::collections::VecDeque;

use orderscope_core::Side;

/// Volume-Synchronized Probability of Informed Trading.
///
/// Reimplemented from first principles (not a dependency — see project plan on why
/// `gregorian-09/orderflow`'s `of_core` was read as reference only, not vendored):
/// accumulate buy/sell volume into fixed-size buckets, emit `|buy-sell|/bucket_qty`
/// per completed bucket, and keep a rolling window of bucket VPIN values for
/// mean/std/toxicity. Uses a `VecDeque` for O(1) sliding-window eviction — the
/// reference implementation's `Vec::remove(0)` is O(n) per eviction, a perf smell
/// deliberately not carried over.
pub struct VpinTracker {
    bucket_qty: f64,
    cur_buy: f64,
    cur_sell: f64,
    history: VecDeque<f64>,
    max_buckets: usize,
    toxicity_threshold: f64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct VpinSnapshot {
    pub vpin: f64,
    pub vpin_mean: f64,
    pub vpin_std: f64,
    pub vpin_zscore: f64,
    pub is_toxic: bool,
    pub bucket_count: u64,
}

impl VpinTracker {
    pub fn new(bucket_qty: f64, max_buckets: usize) -> Self {
        Self {
            bucket_qty,
            cur_buy: 0.0,
            cur_sell: 0.0,
            history: VecDeque::with_capacity(max_buckets),
            max_buckets,
            toxicity_threshold: 2.0,
        }
    }

    pub fn on_trade(&mut self, side: Side, qty: f64) {
        if self.bucket_qty <= 0.0 || self.max_buckets == 0 {
            return;
        }
        match side {
            Side::Buy => self.cur_buy += qty,
            Side::Sell => self.cur_sell += qty,
        }

        let total = self.cur_buy + self.cur_sell;
        if total >= self.bucket_qty {
            let vpin = (self.cur_buy - self.cur_sell).abs() / self.bucket_qty;
            if self.history.len() == self.max_buckets {
                self.history.pop_front();
            }
            self.history.push_back(vpin);

            // Carry over excess volume into the next bucket.
            let excess = total - self.bucket_qty;
            let excess_ratio = excess / total.max(1.0);
            self.cur_buy *= excess_ratio;
            self.cur_sell *= excess_ratio;
        }
    }

    pub fn snapshot(&self) -> VpinSnapshot {
        let Some(&latest) = self.history.back() else {
            return VpinSnapshot::default();
        };
        let n = self.history.len() as f64;
        let mean = self.history.iter().sum::<f64>() / n;
        let variance = self.history.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
        let std = variance.sqrt();
        let zscore = if std > 0.0 { (latest - mean) / std } else { 0.0 };

        VpinSnapshot {
            vpin: latest,
            vpin_mean: mean,
            vpin_std: std,
            vpin_zscore: zscore,
            is_toxic: zscore.abs() > self.toxicity_threshold,
            bucket_count: self.history.len() as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_a_bucket_once_volume_threshold_is_reached() {
        let mut t = VpinTracker::new(100.0, 10);
        assert_eq!(t.snapshot().bucket_count, 0);
        t.on_trade(Side::Buy, 60.0);
        t.on_trade(Side::Sell, 40.0);
        let snap = t.snapshot();
        assert_eq!(snap.bucket_count, 1);
        assert!((snap.vpin - 0.2).abs() < 1e-9); // |60-40|/100
    }

    #[test]
    fn perfectly_balanced_flow_has_zero_vpin() {
        let mut t = VpinTracker::new(100.0, 10);
        t.on_trade(Side::Buy, 50.0);
        t.on_trade(Side::Sell, 50.0);
        assert_eq!(t.snapshot().vpin, 0.0);
    }
}
