//! M1: standalone CLI validating the Finnhub WS adapter against live data.
//!
//! Direct Rust port of the connect+parse step already validated in
//! `finnhub_flow/live_cvd.py`. Run with: `cargo run --example print_trades -- AMD`

use futures_util::StreamExt;
use orderscope_core::{Event, Symbol};
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

    println!("In ascolto sui trade live di {:?}... Ctrl+C per fermare.", symbols.iter().map(Symbol::as_str).collect::<Vec<_>>());

    let mut stream = ws::connect(FinnhubWsConfig { api_key, symbols });

    while let Some(event) = stream.next().await {
        match event {
            Event::Connected(streams) => println!("connesso: {streams:?}"),
            Event::Disconnected(_, reason) => println!("disconnesso: {reason}"),
            Event::TradesReceived(kind, trades) => {
                for t in trades.iter() {
                    println!(
                        "{:?} {:>10.2} size={:<8.0} conditions={:?}",
                        kind,
                        t.price.to_f64(),
                        t.qty.0,
                        t.conditions.codes(),
                    );
                }
            }
        }
    }
}
