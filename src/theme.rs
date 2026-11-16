//! Color tokens for the app, taken from a validated dark palette (checked with
//! the project's `dataviz` skill validator — CVD/colorblind separation, contrast
//! vs. the dark surface — rather than picked by eye). Bullish/bearish uses the
//! palette's categorical green/red slots (validated pair, protan ΔE 8.6), not the
//! status "good/critical" greens/reds, which fail colorblind separation as a pair
//! (deutan ΔE 4.1) — status colors are reserved for actual state (connection,
//! VPIN toxicity), never reused as a second series encoding.

use iced::Color;

pub fn bullish() -> Color {
    Color::from_rgb8(0x00, 0x83, 0x00)
}
pub fn bearish() -> Color {
    Color::from_rgb8(0xe6, 0x67, 0x67)
}
pub fn cvd_line() -> Color {
    Color::from_rgb8(0x39, 0x87, 0xe5)
}
pub fn poc() -> Color {
    Color::from_rgb8(0xc9, 0x85, 0x00)
}

pub fn status_good() -> Color {
    Color::from_rgb8(0x0c, 0xa3, 0x0c)
}
pub fn status_warning() -> Color {
    Color::from_rgb8(0xfa, 0xb2, 0x19)
}
pub fn status_critical() -> Color {
    Color::from_rgb8(0xd0, 0x3b, 0x3b)
}

pub fn surface() -> Color {
    Color::from_rgb8(0x1a, 0x1a, 0x19)
}
pub fn page() -> Color {
    Color::from_rgb8(0x0d, 0x0d, 0x0d)
}
pub fn text_primary() -> Color {
    Color::from_rgb8(0xff, 0xff, 0xff)
}
pub fn text_secondary() -> Color {
    Color::from_rgb8(0xc3, 0xc2, 0xb7)
}
pub fn text_muted() -> Color {
    Color::from_rgb8(0x89, 0x87, 0x81)
}
pub fn gridline() -> Color {
    Color::from_rgb8(0x2c, 0x2c, 0x2a)
}
pub fn axis() -> Color {
    Color::from_rgb8(0x38, 0x38, 0x35)
}
pub fn border() -> Color {
    Color { r: 1.0, g: 1.0, b: 1.0, a: 0.10 }
}
