//! M8-M9: two-pane dashboard (footprint+CVD, insider/13F/congress) built on iced's
//! `pane_grid`, with pane content dispatched through the `PaneContent` trait
//! (see `pane/mod.rs`) instead of a closed enum.

mod chart;
mod pane;
mod theme;

use std::time::{Duration, Instant};

use iced::widget::{container, pane_grid, text, PaneGrid};
use iced::{Border, Element, Length, Subscription, Task};

use orderscope_analytics::classifier::TradeClassifier;
use orderscope_analytics::institutional_flow::InstitutionalFlowTracker;
use orderscope_analytics::vpin::VpinTracker;
use orderscope_core::{Event as CoreEvent, PriceStep, Side, Symbol, UnixMs};
use orderscope_ingest::finnhub::rest::{
    CongressionalTrade, FetchError, FinnhubRestClient, InsiderTransaction, InstitutionalHolding,
};
use orderscope_ingest::finnhub::ws::{self, FinnhubWsConfig};
use orderscope_model::{ClassifiedTrade, CvdTimeSeries, FootprintBar, TimeSeries};
use pane::footprint_cvd::FootprintCvdPane;
use pane::insider::{InsiderPane, InsiderPanelState};
use pane::{PaneContent, PaneKind};

/// 1-minute bars, 5-cent footprint price buckets — reasonable defaults for a
/// single-digit-to-triple-digit-dollar US equity; will become configurable per
/// symbol once multi-symbol support lands (M11).
const BAR_INTERVAL_MS: u64 = 60_000;
const FOOTPRINT_TICK: f64 = 0.05;

/// Insider/13F/congressional data is legally lagged 2-45 days; nothing is gained
/// by polling faster than this.
const INSIDER_REFRESH_SECS: u64 = 1800;

/// VPIN bucket size in shares and rolling-window length — reasonable starting
/// defaults, not derived from the symbol's real average daily volume (would need
/// an extra REST call we don't make yet). Same caveat for the large-print
/// threshold: a fixed share count, not tuned per symbol.
const VPIN_BUCKET_QTY: f64 = 5_000.0;
const VPIN_ROLLING_BUCKETS: usize = 30;
const LARGE_TRADE_THRESHOLD_QTY: f64 = 500.0;
const LARGE_TRADE_WINDOW: usize = 20;

struct App {
    symbol: Symbol,
    ws_api_key: String,
    rest_client: FinnhubRestClient,

    classifier: TradeClassifier,
    vpin: VpinTracker,
    inst_flow: InstitutionalFlowTracker,
    series: TimeSeries<FootprintBar>,
    cvd_series: CvdTimeSeries,
    cvd: f64,
    trade_count: u64,
    last_trade: Option<(f64, f64)>,
    status: String,
    connected_at: Option<Instant>,
    last_event_at: Option<Instant>,

    insider_state: InsiderPanelState,

    panes: pane_grid::State<PaneKind>,
}

#[derive(Debug, Clone)]
enum Message {
    MarketEvent(CoreEvent),
    Tick,
    RefreshInsiderData,
    InsiderDataLoaded(
        Result<Vec<InsiderTransaction>, FetchError>,
        Result<Vec<InstitutionalHolding>, FetchError>,
        Result<Vec<CongressionalTrade>, FetchError>,
    ),
}

fn format_elapsed(since: Option<Instant>) -> String {
    match since {
        Some(instant) => {
            let secs = instant.elapsed().as_secs();
            format!("{:02}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
        }
        None => "mai".to_string(),
    }
}

impl App {
    fn new(ws_api_key: String, rest_api_key: String, symbol: Symbol) -> (Self, Task<Message>) {
        let (mut panes, footprint_pane) = pane_grid::State::new(PaneKind::FootprintCvd);
        panes
            .split(pane_grid::Axis::Vertical, footprint_pane, PaneKind::Insider)
            .expect("splitting the initial pane always succeeds");

        (
            Self {
                symbol,
                ws_api_key,
                rest_client: FinnhubRestClient::new(rest_api_key),
                classifier: TradeClassifier::new(),
                vpin: VpinTracker::new(VPIN_BUCKET_QTY, VPIN_ROLLING_BUCKETS),
                inst_flow: InstitutionalFlowTracker::new(LARGE_TRADE_THRESHOLD_QTY, LARGE_TRADE_WINDOW),
                series: TimeSeries::new(BAR_INTERVAL_MS, PriceStep::from_f64(FOOTPRINT_TICK)),
                cvd_series: CvdTimeSeries::default(),
                cvd: 0.0,
                trade_count: 0,
                last_trade: None,
                status: "connessione in corso...".to_string(),
                connected_at: None,
                last_event_at: None,
                insider_state: InsiderPanelState::default(),
                panes,
            },
            Task::done(Message::RefreshInsiderData),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {}
            Message::MarketEvent(CoreEvent::Connected(_)) => {
                self.status = "connesso".to_string();
                self.connected_at = Some(Instant::now());
            }
            Message::MarketEvent(CoreEvent::Disconnected(_, reason)) => {
                self.status = format!("disconnesso: {reason} (riconnessione automatica...)");
                self.connected_at = None;
            }
            Message::MarketEvent(CoreEvent::TradesReceived(_, trades)) => {
                for t in trades.iter() {
                    let side = self.classifier.classify(t.price);
                    let signed = if side == Side::Buy { t.qty.0 } else { -t.qty.0 };
                    self.cvd += signed;
                    self.trade_count += 1;
                    self.last_trade = Some((t.price.to_f64(), t.qty.0));

                    self.series.ingest(&ClassifiedTrade { time: t.time, price: t.price, qty: t.qty, side });
                    self.vpin.on_trade(side, t.qty.0);
                    self.inst_flow.on_trade(t.qty.0, side);
                }
                self.cvd_series.rebuild_from(&self.series.buckets);
                self.last_event_at = Some(Instant::now());
            }
            Message::RefreshInsiderData => {
                self.insider_state.loading = true;
                let client = self.rest_client.clone();
                let symbol = self.symbol.as_str().to_string();
                return Task::perform(
                    async move {
                        // Independent, non-short-circuiting: one endpoint failing
                        // (institutional/ownership reliably 403s on the free plan)
                        // must not blank out the other two, which typically succeed.
                        let insider = client.insider_transactions(&symbol).await;
                        let institutional = client.institutional_ownership(&symbol).await;
                        let congress = client.congressional_trading(&symbol).await;
                        (insider, institutional, congress)
                    },
                    |(insider, institutional, congress)| {
                        Message::InsiderDataLoaded(insider, institutional, congress)
                    },
                );
            }
            Message::InsiderDataLoaded(insider, institutional, congress) => {
                self.insider_state.loading = false;

                match insider {
                    Ok(v) => {
                        self.insider_state.insider_transactions = v;
                        self.insider_state.insider_error = None;
                    }
                    Err(e) => self.insider_state.insider_error = Some(e.to_string()),
                }
                match institutional {
                    Ok(v) => {
                        self.insider_state.institutional_holdings = v;
                        self.insider_state.institutional_error = None;
                    }
                    Err(e) => self.insider_state.institutional_error = Some(e.to_string()),
                }
                match congress {
                    Ok(v) => {
                        self.insider_state.congressional_trades = v;
                        self.insider_state.congressional_error = None;
                    }
                    Err(e) => self.insider_state.congressional_error = Some(e.to_string()),
                }
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let pane_grid = PaneGrid::new(&self.panes, |_id, kind, _is_maximized| {
            let content: Box<dyn PaneContent> = match kind {
                PaneKind::FootprintCvd => Box::new(self.build_footprint_pane()),
                PaneKind::Insider => Box::new(InsiderPane {
                    symbol: self.symbol.as_str().to_string(),
                    state: self.insider_state.clone(),
                }),
            };
            let title = content.title();

            let title_bar = pane_grid::TitleBar::new(
                text(title).size(12).color(theme::text_secondary()),
            )
            .padding(10)
            .style(|_theme| {
                container::Style::default()
                    .background(theme::page())
                    .border(Border { color: theme::border(), width: 1.0, radius: 0.0.into() })
            });

            pane_grid::Content::new(content.view()).title_bar(title_bar).style(|_theme| {
                container::Style::default()
                    .background(theme::surface())
                    .border(Border { color: theme::border(), width: 1.0, radius: 0.0.into() })
            })
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .spacing(2);

        container(pane_grid)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_theme| container::Style::default().background(theme::page()))
            .into()
    }

    fn build_footprint_pane(&self) -> FootprintCvdPane {
        // Footprint chart has its own scroll-to-zoom/drag-to-pan state, so it needs
        // more history available than just the default visible window (30 bars).
        let bars = self.series.last_n(300);
        let cvd_points: Vec<(UnixMs, f64)> = bars
            .iter()
            .map(|(t, _)| (*t, self.cvd_series.cumulative.get(t).copied().unwrap_or(0.0)))
            .collect();

        let validation = format!(
            "{}  ·  connesso da {}  ·  ultimo evento {} fa",
            chrono::Local::now().format("%H:%M:%S"),
            format_elapsed(self.connected_at),
            format_elapsed(self.last_event_at),
        );

        let status_color = if self.connected_at.is_some() {
            theme::status_good()
        } else if self.status.starts_with("disconnesso") {
            theme::status_critical()
        } else {
            theme::status_warning()
        };

        let vpin = self.vpin.snapshot();
        let flow = self.inst_flow.snapshot();
        let analytics_badge = if vpin.bucket_count == 0 && flow.large_trade_count == 0 {
            "VPIN: n/d (in accumulo)  ·  large prints: n/d".to_string()
        } else {
            format!(
                "VPIN: {:.2}{}  ({} bucket)  ·  large prints ({}+ az.): {} · buy {:.0}%",
                vpin.vpin,
                if vpin.is_toxic { " ⚠ tossico" } else { "" },
                vpin.bucket_count,
                LARGE_TRADE_THRESHOLD_QTY as u64,
                flow.large_trade_count,
                flow.buy_ratio * 100.0,
            )
        };

        FootprintCvdPane {
            symbol: self.symbol.clone(),
            bars: bars.into_iter().map(|(t, b)| (t, b.clone())).collect(),
            cvd_points,
            cvd_total: self.cvd,
            last_trade: self.last_trade,
            trade_count: self.trade_count,
            status: self.status.clone(),
            status_color,
            validation,
            analytics_badge,
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        let config = FinnhubWsConfig { api_key: self.ws_api_key.clone(), symbols: vec![self.symbol.clone()] };

        let trades = Subscription::run_with_id("finnhub-trades", ws::connect(config)).map(Message::MarketEvent);
        let tick = iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick);
        let insider_refresh =
            iced::time::every(Duration::from_secs(INSIDER_REFRESH_SECS)).map(|_| Message::RefreshInsiderData);

        Subscription::batch([trades, tick, insider_refresh])
    }
}

fn main() -> iced::Result {
    dotenvy::from_filename("finnhub_flow/.env").ok();
    dotenvy::from_filename("../finnhub_flow/.env").ok();
    dotenvy::dotenv().ok();

    let api_key = std::env::var("FINNHUB_API_KEY").expect("set FINNHUB_API_KEY in the environment or a .env file");
    let symbol = std::env::args()
        .nth(1)
        .map(|s| Symbol::from(s.as_str()))
        .unwrap_or_else(|| Symbol::from("AAPL"));

    iced::application("orderscope", App::update, App::view)
        .subscription(App::subscription)
        .theme(|_state| iced::Theme::Dark)
        .window_size(iced::Size::new(1200.0, 780.0))
        .run_with(move || App::new(api_key.clone(), api_key.clone(), symbol.clone()))
}
