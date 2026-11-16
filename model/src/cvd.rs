use std::collections::BTreeMap;

use orderscope_core::UnixMs;

use crate::footprint::FootprintBar;

/// Cumulative Volume Delta, derived as a prefix-sum over a `TimeSeries<FootprintBar>`
/// rather than tracked as an independent running counter — avoids double-bookkeeping
/// and any risk of the two drifting apart. Cheap to rebuild at single-equity trade
/// volumes.
#[derive(Debug, Clone, Default)]
pub struct CvdTimeSeries {
    pub cumulative: BTreeMap<UnixMs, f64>,
}

impl CvdTimeSeries {
    pub fn rebuild_from(&mut self, bars: &BTreeMap<UnixMs, FootprintBar>) {
        let mut running = 0.0;
        self.cumulative.clear();
        for (t, bar) in bars {
            running += bar.volume.delta();
            self.cumulative.insert(*t, running);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::footprint::Volume;

    #[test]
    fn accumulates_delta_across_bars_in_time_order() {
        let mut bars = BTreeMap::new();
        let b1 = FootprintBar { volume: Volume::BuySell(10.0, 4.0), ..Default::default() }; // delta +6
        let b2 = FootprintBar { volume: Volume::BuySell(2.0, 9.0), ..Default::default() }; // delta -7
        bars.insert(UnixMs(0), b1);
        bars.insert(UnixMs(60_000), b2);

        let mut cvd = CvdTimeSeries::default();
        cvd.rebuild_from(&bars);

        assert_eq!(cvd.cumulative[&UnixMs(0)], 6.0);
        assert_eq!(cvd.cumulative[&UnixMs(60_000)], -1.0);
    }
}
