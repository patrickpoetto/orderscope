//! Off-exchange/dark-pool volume tracker.
//!
//! Stub only (M11, deferred): we do not yet have a verified interpretation of which
//! Finnhub trade condition code (in `Trade::conditions`) reliably marks an off-exchange
//! print. Two secondary sources disagreed on what code 24 means (see project plan), and
//! condition-code-to-integer mappings are vendor-specific rather than standardized, so
//! this needs live verification against known trades before it can be implemented.
