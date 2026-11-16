use std::sync::Arc;

/// Fixed-point price, stored as an integer count of 1/10_000ths of a dollar.
///
/// Backed by an integer (not `f64`) because `Price` is used as a `HashMap`/`BTreeMap`
/// key for footprint price-level bucketing, which needs clean `Eq`/`Hash`/`Ord`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Price(i64);

const PRICE_SCALE: f64 = 10_000.0;

impl Price {
    pub fn from_f64(value: f64) -> Self {
        Self((value * PRICE_SCALE).round() as i64)
    }

    pub fn to_f64(self) -> f64 {
        self.0 as f64 / PRICE_SCALE
    }

    /// Rounds this price down to the nearest multiple of `step`, bucketing it
    /// into the price level used for footprint clustering.
    pub fn round_to_step(self, step: PriceStep) -> Price {
        let step = step.0.max(1);
        Price((self.0.div_euclid(step)) * step)
    }
}

/// The width of a footprint price bucket, in the same fixed-point units as [`Price`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PriceStep(i64);

impl PriceStep {
    pub fn from_f64(value: f64) -> Self {
        Self(((value * PRICE_SCALE).round() as i64).max(1))
    }
}

/// Trade/order size in shares. Kept as `f64` since Finnhub trade sizes can be
/// fractional (fractional-share trading) and it is never used as a map key.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Qty(pub f64);

impl Qty {
    pub const ZERO: Qty = Qty(0.0);
}

impl std::ops::Add for Qty {
    type Output = Qty;
    fn add(self, rhs: Qty) -> Qty {
        Qty(self.0 + rhs.0)
    }
}

impl std::ops::AddAssign for Qty {
    fn add_assign(&mut self, rhs: Qty) {
        self.0 += rhs.0;
    }
}

/// Milliseconds since the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnixMs(pub u64);

impl UnixMs {
    /// Floors this timestamp down to the start of the enclosing `interval_ms` bucket.
    pub fn floor_to(self, interval_ms: u64) -> UnixMs {
        UnixMs(self.0 - self.0 % interval_ms.max(1))
    }
}

/// A ticker symbol. Cheap to clone (reference-counted string).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Symbol(Arc<str>);

impl Symbol {
    pub fn new(s: impl Into<Arc<str>>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for Symbol {
    fn from(s: &str) -> Self {
        Self::new(s.to_uppercase())
    }
}
