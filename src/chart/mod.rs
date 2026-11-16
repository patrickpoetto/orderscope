//! Candlestick + footprint rendering (M5/M6), GPU-drawn via iced's `canvas::Program`,
//! autoscaled to fit the visible window on every frame.
//!
//! Interactive pan/zoom (scroll wheel = zoom, click-drag = pan through history)
//! lives in `ChartState`, following Flowsurface's `Chart`/`ViewState` pattern
//! conceptually (persistent pan/zoom state owned by the canvas program), not its
//! code — reimplemented from scratch. Because zoom now changes how many bars are
//! on screen, per-price-level footprint numbers (M6) light up reliably once the
//! user zooms into a tight-enough window; zoomed out, only the point-of-control
//! marker draws, exactly as designed.

pub mod cvd;

use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke};
use iced::{mouse, Point, Rectangle, Renderer, Size, Theme};

use orderscope_core::UnixMs;
use orderscope_model::FootprintBar;

use crate::theme;

/// Minimum pixel width/row-height for a bar's per-level buy/sell numbers to be legible.
const MIN_BODY_WIDTH_FOR_TEXT: f32 = 46.0;
const MIN_ROW_HEIGHT_FOR_TEXT: f32 = 10.0;
const MIN_VISIBLE_BARS: usize = 4;

/// Persistent pan/zoom state, owned by iced's canvas runtime across frames (not
/// rebuilt each `view()` call, unlike the `FootprintChart`/`FootprintBar` data
/// itself). `visible_bars` is the zoom level (how many bars fit on screen);
/// `offset` is how many bars back from the live edge the view has been panned.
#[derive(Debug, Clone, Copy)]
pub struct ChartState {
    pub visible_bars: usize,
    pub offset: usize,
    dragging: bool,
    last_drag_x: f32,
}

impl Default for ChartState {
    fn default() -> Self {
        Self { visible_bars: 30, offset: 0, dragging: false, last_drag_x: 0.0 }
    }
}

pub struct FootprintChart {
    pub bars: Vec<(UnixMs, FootprintBar)>,
}

impl<Message> canvas::Program<Message> for FootprintChart {
    type State = ChartState;

    fn update(
        &self,
        state: &mut Self::State,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (canvas::event::Status, Option<Message>) {
        let canvas::Event::Mouse(mouse_event) = event else {
            return (canvas::event::Status::Ignored, None);
        };

        let total = self.bars.len().max(1);
        let max_offset = total.saturating_sub(state.visible_bars);

        match mouse_event {
            mouse::Event::WheelScrolled { delta } if cursor.is_over(bounds) => {
                let dy = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => y,
                    mouse::ScrollDelta::Pixels { y, .. } => y / 40.0,
                };
                if dy != 0.0 {
                    // `floor` when shrinking / `ceil` when growing guarantees each
                    // scroll step actually moves `visible_bars`, even near the
                    // minimum — plain `.round()` can get stuck oscillating at a
                    // fixed point (e.g. 5 * 0.9 = 4.5, which rounds back up to 5)
                    // and never reach `MIN_VISIBLE_BARS`.
                    let zooming_in = dy > 0.0;
                    let scaled = state.visible_bars as f32 * if zooming_in { 0.9 } else { 1.1 };
                    let new_visible =
                        (if zooming_in { scaled.floor() } else { scaled.ceil() } as usize)
                            .clamp(MIN_VISIBLE_BARS, total);
                    state.visible_bars = new_visible;
                    state.offset = state.offset.min(total.saturating_sub(state.visible_bars));
                    (canvas::event::Status::Captured, None)
                } else {
                    (canvas::event::Status::Ignored, None)
                }
            }
            mouse::Event::ButtonPressed(mouse::Button::Left) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    state.dragging = true;
                    state.last_drag_x = pos.x;
                    (canvas::event::Status::Captured, None)
                } else {
                    (canvas::event::Status::Ignored, None)
                }
            }
            mouse::Event::ButtonReleased(mouse::Button::Left) => {
                state.dragging = false;
                (canvas::event::Status::Captured, None)
            }
            mouse::Event::CursorMoved { .. } if state.dragging => {
                if let Some(pos) = cursor.position_in(bounds) {
                    let px_per_bar = bounds.width / state.visible_bars.max(1) as f32;
                    let dx = pos.x - state.last_drag_x;
                    let bar_delta = (dx / px_per_bar.max(0.01)) as isize;
                    if bar_delta != 0 {
                        let new_offset = (state.offset as isize + bar_delta).clamp(0, max_offset as isize) as usize;
                        state.offset = new_offset;
                        state.last_drag_x = pos.x;
                    }
                    (canvas::event::Status::Captured, None)
                } else {
                    (canvas::event::Status::Ignored, None)
                }
            }
            _ => (canvas::event::Status::Ignored, None),
        }
    }

    fn mouse_interaction(&self, state: &Self::State, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        if state.dragging {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(bounds) {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::default()
        }
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        let total = self.bars.len();
        let visible_bars = state.visible_bars.clamp(MIN_VISIBLE_BARS, total.max(MIN_VISIBLE_BARS));
        let end = total.saturating_sub(state.offset);
        let start = end.saturating_sub(visible_bars);
        let window = self.bars.get(start..end).unwrap_or(&[]);

        let complete_bars: Vec<_> =
            window.iter().filter_map(|(t, b)| Some((*t, b, b.open?, b.high?, b.low?, b.close?))).collect();

        if complete_bars.is_empty() {
            return vec![frame.into_geometry()];
        }

        let high = complete_bars.iter().map(|(_, _, _, h, _, _)| h.to_f64()).fold(f64::MIN, f64::max);
        let low = complete_bars.iter().map(|(_, _, _, _, l, _)| l.to_f64()).fold(f64::MAX, f64::min);
        let price_range = (high - low).max(0.01);

        let margin = 8.0;
        let plot_height = (bounds.height - margin * 2.0).max(1.0);
        let plot_width = (bounds.width - margin * 2.0).max(1.0);

        let n = complete_bars.len();
        let slot_width = plot_width / n as f32;
        let body_width = (slot_width * 0.6).max(1.0);

        let y_for = |price: f64| -> f32 {
            let frac = (price - low) / price_range;
            margin + plot_height * (1.0 - frac as f32)
        };

        // Faint horizontal gridlines at high/mid/low.
        for price in [low, low + (high - low) / 2.0, high] {
            let y = y_for(price);
            frame.stroke(
                &Path::line(Point::new(0.0, y), Point::new(bounds.width, y)),
                Stroke::default().with_color(theme::gridline()).with_width(0.5),
            );
            frame.fill_text(canvas::Text {
                content: format!("{price:.2}"),
                position: Point::new(2.0, y - 10.0),
                color: theme::text_muted(),
                size: 10.0.into(),
                ..Default::default()
            });
        }

        for (i, (_, bar, open, high, low, close)) in complete_bars.iter().enumerate() {
            let cx = margin + slot_width * (i as f32 + 0.5);
            let bullish = close.to_f64() >= open.to_f64();
            let color = if bullish { theme::bullish() } else { theme::bearish() };

            // Wick: full high-low range.
            frame.stroke(
                &Path::line(Point::new(cx, y_for(high.to_f64())), Point::new(cx, y_for(low.to_f64()))),
                Stroke::default().with_color(color).with_width(1.5),
            );

            // Body: open-close range.
            let body_top = y_for(open.to_f64().max(close.to_f64()));
            let body_bottom = y_for(open.to_f64().min(close.to_f64()));
            let body_height = (body_bottom - body_top).max(1.5);
            frame.fill_rectangle(
                Point::new(cx - body_width / 2.0, body_top),
                Size::new(body_width, body_height),
                color,
            );

            // M6: point-of-control marker — a short gold tick at the bar's
            // highest-volume price level, drawn regardless of zoom level.
            if let Some(poc) = bar.poc {
                let y = y_for(poc.to_f64());
                frame.stroke(
                    &Path::line(
                        Point::new(cx - body_width / 2.0 - 2.0, y),
                        Point::new(cx + body_width / 2.0 + 2.0, y),
                    ),
                    Stroke::default().with_color(theme::poc()).with_width(2.0),
                );
            }

            // M6: per-price-level buy/sell numbers, only once there's enough room
            // (both bar width and per-tick row height) for them to be legible —
            // scroll-to-zoom into a tighter window to make these appear.
            let mut levels: Vec<_> = bar.levels.iter().collect();
            levels.sort_by(|a, b| b.0.cmp(a.0));
            if !levels.is_empty() {
                let bar_pixel_span = (y_for(low.to_f64()) - y_for(high.to_f64())).abs();
                let row_height = bar_pixel_span / levels.len() as f32;
                if body_width >= MIN_BODY_WIDTH_FOR_TEXT && row_height >= MIN_ROW_HEIGHT_FOR_TEXT {
                    for (price, grouped) in &levels {
                        let y = y_for(price.to_f64());
                        frame.fill_text(canvas::Text {
                            content: format!("{:.0}", grouped.buy_qty),
                            position: Point::new(cx - body_width / 2.0, y - row_height / 2.0),
                            color: theme::bullish(),
                            size: (row_height * 0.7).min(11.0).into(),
                            ..Default::default()
                        });
                        frame.fill_text(canvas::Text {
                            content: format!("{:.0}", grouped.sell_qty),
                            position: Point::new(cx + 2.0, y - row_height / 2.0),
                            color: theme::bearish(),
                            size: (row_height * 0.7).min(11.0).into(),
                            ..Default::default()
                        });
                    }
                }
            }
        }

        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::Rectangle;

    fn chart_with_bars(n: usize) -> FootprintChart {
        FootprintChart { bars: (0..n).map(|i| (UnixMs(i as u64), FootprintBar::default())).collect() }
    }

    fn bounds() -> Rectangle {
        Rectangle { x: 0.0, y: 0.0, width: 300.0, height: 200.0 }
    }

    #[test]
    fn scroll_up_zooms_in_reducing_visible_bars() {
        let chart = chart_with_bars(100);
        let mut state = ChartState::default();
        assert_eq!(state.visible_bars, 30);

        canvas::Program::<()>::update(
            &chart,
            &mut state,
            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 } }),
            bounds(),
            mouse::Cursor::Available(Point::new(150.0, 100.0)),
        );

        assert_eq!(state.visible_bars, 27); // 30 * 0.9, rounded
    }

    #[test]
    fn scroll_down_zooms_out_increasing_visible_bars() {
        let chart = chart_with_bars(100);
        let mut state = ChartState::default();

        canvas::Program::<()>::update(
            &chart,
            &mut state,
            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta: mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 } }),
            bounds(),
            mouse::Cursor::Available(Point::new(150.0, 100.0)),
        );

        assert_eq!(state.visible_bars, 33); // 30 * 1.1, rounded
    }

    #[test]
    fn zoom_never_goes_below_the_minimum_or_above_total_bars() {
        let chart = chart_with_bars(10);
        let mut state = ChartState { visible_bars: 10, ..ChartState::default() };

        // Zoom out repeatedly: must clamp at `total` (10), never exceed it.
        for _ in 0..10 {
            canvas::Program::<()>::update(
                &chart,
                &mut state,
                canvas::Event::Mouse(mouse::Event::WheelScrolled {
                    delta: mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
                }),
                bounds(),
                mouse::Cursor::Available(Point::new(150.0, 100.0)),
            );
        }
        assert_eq!(state.visible_bars, 10);

        // Zoom in repeatedly: must clamp at MIN_VISIBLE_BARS, never go lower.
        for _ in 0..40 {
            canvas::Program::<()>::update(
                &chart,
                &mut state,
                canvas::Event::Mouse(mouse::Event::WheelScrolled { delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 } }),
                bounds(),
                mouse::Cursor::Available(Point::new(150.0, 100.0)),
            );
        }
        assert_eq!(state.visible_bars, MIN_VISIBLE_BARS);
    }

    #[test]
    fn dragging_right_pans_back_into_history() {
        let chart = chart_with_bars(100); // max_offset = 100 - 30 = 70
        let mut state = ChartState::default();

        canvas::Program::<()>::update(
            &chart,
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds(),
            mouse::Cursor::Available(Point::new(150.0, 100.0)),
        );
        assert!(state.dragging);

        // px_per_bar = 300 / 30 = 10; dragging +100px right = +10 bars of offset.
        canvas::Program::<()>::update(
            &chart,
            &mut state,
            canvas::Event::Mouse(mouse::Event::CursorMoved { position: Point::new(250.0, 100.0) }),
            bounds(),
            mouse::Cursor::Available(Point::new(250.0, 100.0)),
        );
        assert_eq!(state.offset, 10);

        canvas::Program::<()>::update(
            &chart,
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            bounds(),
            mouse::Cursor::Available(Point::new(250.0, 100.0)),
        );
        assert!(!state.dragging);
    }

    #[test]
    fn offset_never_goes_negative_when_dragging_past_the_live_edge() {
        let chart = chart_with_bars(100);
        let mut state = ChartState::default(); // offset already 0, at the live edge

        canvas::Program::<()>::update(
            &chart,
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds(),
            mouse::Cursor::Available(Point::new(150.0, 100.0)),
        );
        canvas::Program::<()>::update(
            &chart,
            &mut state,
            canvas::Event::Mouse(mouse::Event::CursorMoved { position: Point::new(50.0, 100.0) }), // drag left
            bounds(),
            mouse::Cursor::Available(Point::new(50.0, 100.0)),
        );

        assert_eq!(state.offset, 0);
    }
}
