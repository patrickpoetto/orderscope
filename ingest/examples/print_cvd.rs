//! M2: adds tick-rule classification + a running Cumulative Volume Delta on top of
//! M1's raw trade printer. The sticky-tick-rule logic must match
//! `finnhub_flow/live_cvd.py` exactly — this is the regression check against that
//! already-validated Python prototype before any UI exists.
//!
//! Run with: `cargo run -p orderscope-ingest --example print_cvd -- AMD`

use std::collections::HashMap;

use futures_util::StreamExt;
use orderscope_analytics::classifier::TradeClassifier;
use orderscope_core::{Event, Side, Symbol};
use orderscope_ingest::finnhub::ws::{self, FinnhubWsConfig};

#[tokio::main]
async fn main() {
    dotenvy::from_filename("../finnhub_flow/.env").ok();
    dotenvy::dotenv().ok();

    let api_key = std::env::var("FINNHUB_API_KEY").expect("set FINNHUB_API_KEY in the environment or a .env file");

    let symbols: Vec<Symbol> = std::env::args()
        .skip(1)
        .map(|s| Symbol::from(s.as_str()))
        .collect();
    let symbols = if symbols.is_empty() { vec![Symbol::from("AAPL")] } else { symbols };

    println!(
        "In ascolto sui trade live di {:?}... Ctrl+C per fermare.",
        symbols.iter().map(Symbol::as_str).collect::<Vec<_>>()
    );

    let mut classifiers: HashMap<Symbol, TradeClassifier> = HashMap::new();
    let mut cvd: HashMap<Symbol, f64> = HashMap::new();

    let mut stream = ws::connect(FinnhubWsConfig { api_key, symbols });

    while let Some(event) = stream.next().await {
        match event {
            Event::Connected(_) => println!("connesso"),
            Event::Disconnected(_, reason) => println!("disconnesso: {reason}"),
            Event::TradesReceived(kind, trades) => {
                let orderscope_core::StreamKind::Trades { symbol } = kind;
                let classifier = classifiers.entry(symbol.clone()).or_default();
                let running = cvd.entry(symbol.clone()).or_insert(0.0);

                for t in trades.iter() {
                    let side = classifier.classify(t.price);
                    let signed = if side == Side::Buy { t.qty.0 } else { -t.qty.0 };
                    *running += signed;

                    let arrow = if side == Side::Buy { "+" } else { "-" };
                    println!(
                        "{symbol} {:>10.2} size={:<8.0} {arrow} CVD={:>+12.0}",
                        t.price.to_f64(),
                        t.qty.0,
                        running,
                    );
                }
            }
        }
    }
}
