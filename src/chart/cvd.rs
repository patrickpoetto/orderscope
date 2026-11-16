//! M7: CVD sub-pane. Draws the cumulative-delta step line for the same bars shown
//! in the footprint pane above it, so the two stay visually time-locked (same bar
//! count/order) without yet needing a shared interactive `ViewState` (no pan/zoom
//! exists on either pane today — see chart/mod.rs).

use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke};
use iced::{Point, Rectangle, Renderer, Theme};

use orderscope_core::UnixMs;

use crate::theme;

pub struct CvdChart {
    /// Cumulative delta per bar, in the same left-to-right order as the footprint pane.
    pub points: Vec<(UnixMs, f64)>,
}

impl<Message> canvas::Program<Message> for CvdChart {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        if self.points.is_empty() {
            return vec![frame.into_geometry()];
        }

        let high = self.points.iter().map(|(_, v)| *v).fold(f64::MIN, f64::max).max(0.0);
        let low = self.points.iter().map(|(_, v)| *v).fold(f64::MAX, f64::min).min(0.0);
        let range = (high - low).max(1.0);

        let margin = 8.0;
        let plot_height = (bounds.height - margin * 2.0).max(1.0);
        let plot_width = (bounds.width - margin * 2.0).max(1.0);
        let n = self.points.len();
        let slot_width = plot_width / n as f32;

        let y_for = |value: f64| -> f32 {
            let frac = (value - low) / range;
            margin + plot_height * (1.0 - frac as f32)
        };

        // Zero line, since CVD direction (above/below zero) matters more than its absolute value.
        let zero_y = y_for(0.0);
        frame.stroke(
            &Path::line(Point::new(0.0, zero_y), Point::new(bounds.width, zero_y)),
            Stroke::default().with_color(theme::axis()).with_width(0.5),
        );

        let path = Path::new(|builder| {
            for (i, (_, value)) in self.points.iter().enumerate() {
                let x = margin + slot_width * (i as f32 + 0.5);
                let y = y_for(*value);
                if i == 0 {
                    builder.move_to(Point::new(x, y));
                } else {
                    builder.line_to(Point::new(x, y));
                }
            }
        });
        frame.stroke(&path, Stroke::default().with_color(theme::cvd_line()).with_width(2.0));

        frame.fill_text(canvas::Text {
            content: format!("CVD {:+.0}", self.points.last().map(|(_, v)| *v).unwrap_or(0.0)),
            position: Point::new(2.0, 2.0),
            color: theme::cvd_line(),
            size: 11.0.into(),
            ..Default::default()
        });

        vec![frame.into_geometry()]
    }
}
