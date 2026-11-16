use iced::widget::{column, text};
use iced::Element;

use super::PaneContent;
use crate::theme;
use crate::Message;
use orderscope_ingest::finnhub::rest::{CongressionalTrade, InsiderTransaction, InstitutionalHolding};

/// Positioning-panel state for one symbol. Insider/13F/congressional data is
/// legally lagged (2-45 days depending on filing type — see `ingest::finnhub::rest`),
/// so this only needs to be refreshed on a slow timer, never per-trade.
///
/// Each of the three sections tracks its own loading/error state independently:
/// on the free plan, `institutional_ownership` reliably 403s (premium-only) while
/// the other two succeed, and one section failing must never blank out the others.
#[derive(Debug, Clone, Default)]
pub struct InsiderPanelState {
    pub loading: bool,
    pub insider_transactions: Vec<InsiderTransaction>,
    pub insider_error: Option<String>,
    pub institutional_holdings: Vec<InstitutionalHolding>,
    pub institutional_error: Option<String>,
    pub congressional_trades: Vec<CongressionalTrade>,
    pub congressional_error: Option<String>,
}

pub struct InsiderPane {
    pub symbol: String,
    pub state: InsiderPanelState,
}

impl PaneContent for InsiderPane {
    fn title(&self) -> String {
        format!("Insider / 13F / Congress — {}", self.symbol)
    }

    fn view(self: Box<Self>) -> Element<'static, Message> {
        let mut rows = Vec::new();

        if self.state.loading {
            rows.push(text("caricamento...").size(12).color(theme::text_muted()).into());
        }

        section_header(&mut rows, "Insider (Form 4)");
        section_rows(
            &mut rows,
            &self.state.insider_error,
            self.state.insider_transactions.is_empty(),
            self.state.loading,
        );
        for t in self.state.insider_transactions.iter().take(8) {
            let color = if t.change >= 0.0 { theme::bullish() } else { theme::bearish() };
            rows.push(
                text(format!("{}  {:+.0} az.  {}", t.name, t.change, t.transaction_date))
                    .size(12)
                    .color(color)
                    .into(),
            );
        }

        section_header(&mut rows, "Istituzionali (13F)");
        section_rows(
            &mut rows,
            &self.state.institutional_error,
            self.state.institutional_holdings.is_empty(),
            self.state.loading,
        );
        for h in self.state.institutional_holdings.iter().take(8) {
            rows.push(
                text(format!("{}  {:.0} az.", h.investor_name, h.share))
                    .size(12)
                    .color(theme::text_secondary())
                    .into(),
            );
        }

        section_header(&mut rows, "Congresso (STOCK Act)");
        section_rows(
            &mut rows,
            &self.state.congressional_error,
            self.state.congressional_trades.is_empty(),
            self.state.loading,
        );
        for c in self.state.congressional_trades.iter().take(8) {
            rows.push(
                text(format!("{}  {}  {}", c.representative, c.transaction_type, c.transaction_date))
                    .size(12)
                    .color(theme::text_secondary())
                    .into(),
            );
        }

        column(rows).padding(14).spacing(6).into()
    }
}

fn section_header(rows: &mut Vec<Element<'static, Message>>, label: &'static str) {
    rows.push(text(label).size(13).color(theme::text_primary()).into());
}

fn section_rows(rows: &mut Vec<Element<'static, Message>>, error: &Option<String>, is_empty: bool, loading: bool) {
    if let Some(err) = error {
        rows.push(text(err.clone()).size(12).color(theme::status_warning()).into());
    } else if is_empty && !loading {
        rows.push(text("nessun dato disponibile").size(12).color(theme::text_muted()).into());
    }
}
