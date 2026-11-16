use iced::widget::{canvas, column, row, text};
use iced::{Color, Element, Length};

use orderscope_core::{Symbol, UnixMs};
use orderscope_model::FootprintBar;

use super::PaneContent;
use crate::chart::cvd::CvdChart;
use crate::chart::FootprintChart;
use crate::theme;
use crate::Message;

/// Owned snapshot of `App`'s live trade/footprint/CVD state for one symbol.
/// Built fresh on every `view()` call, same as the raw chart structs were before M8.
pub struct FootprintCvdPane {
    pub symbol: Symbol,
    pub bars: Vec<(UnixMs, FootprintBar)>,
    pub cvd_points: Vec<(UnixMs, f64)>,
    pub cvd_total: f64,
    pub last_trade: Option<(f64, f64)>,
    pub trade_count: u64,
    pub status: String,
    pub status_color: Color,
    pub validation: String,
    pub analytics_badge: String,
}

impl PaneContent for FootprintCvdPane {
    fn title(&self) -> String {
        format!("Footprint — {}", self.symbol)
    }

    fn view(self: Box<Self>) -> Element<'static, Message> {
        let last = self
            .last_trade
            .map(|(p, q)| format!("ultimo trade  {p:.2}  ×{q:.0}"))
            .unwrap_or_else(|| "in attesa del primo trade...".to_string());

        let footprint_chart = FootprintChart { bars: self.bars };
        let cvd_chart = CvdChart { points: self.cvd_points };

        let cvd_color = if self.cvd_total >= 0.0 { theme::bullish() } else { theme::bearish() };

        let status_row = row![
            text("●").size(11).color(self.status_color),
            text(self.status).size(13).color(theme::text_secondary()),
        ]
        .spacing(6);

        let stats_row = row![
            text(last).size(13).color(theme::text_primary()),
            text(format!("trade: {}", self.trade_count)).size(13).color(theme::text_secondary()),
            text(format!("CVD {:+.0}", self.cvd_total)).size(14).color(cvd_color),
        ]
        .spacing(18);

        column![
            status_row,
            stats_row,
            text(self.analytics_badge).size(12).color(theme::text_muted()),
            text("scroll = zoom · trascina = scorri storico").size(10).color(theme::text_muted()),
            canvas(footprint_chart).width(Length::Fill).height(Length::Fixed(280.0)),
            canvas(cvd_chart).width(Length::Fill).height(Length::Fixed(100.0)),
            text(self.validation).size(11).color(theme::text_muted()),
        ]
        .padding(14)
        .spacing(10)
        .into()
    }
}
