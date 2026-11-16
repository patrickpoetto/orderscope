//! Rolling CVD ratio/z-score/divergence stats.
//!
//! Stub only (M10, not yet started): no code lives here yet. Planned to compute a
//! rolling buy/sell ratio and z-score over the `CvdTimeSeries` bars (see
//! `orderscope_model::cvd`) and flag price/CVD divergence (price making a new
//! high/low while CVD does not) — same category of "heuristic, not certainty"
//! caveat as `institutional_flow` and `vpin`.
