use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures_util::stream::BoxStream;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio_tungstenite::tungstenite::Message;

use orderscope_core::{Conditions, Event, Price, Qty, StreamKind, Symbol, Trade, UnixMs};

#[derive(Debug, Clone)]
pub struct FinnhubWsConfig {
    pub api_key: String,
    pub symbols: Vec<Symbol>,
}

#[derive(Deserialize)]
struct RawMsg {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    data: Vec<RawTrade>,
}

#[derive(Deserialize)]
struct RawTrade {
    s: String,
    p: f64,
    v: f64,
    t: u64,
    #[serde(default)]
    c: Vec<u16>,
}

/// Connects to Finnhub's free trade WebSocket and yields normalized [`Event`]s.
///
/// Reconnects with exponential backoff (1s, 2s, 4s, ... capped at 30s) on any
/// error or close, resetting the backoff after a successful (re)connection.
pub fn connect(config: FinnhubWsConfig) -> BoxStream<'static, Event> {
    let stream = async_stream::stream! {
        let streams: Arc<[StreamKind]> = config
            .symbols
            .iter()
            .map(|s| StreamKind::Trades { symbol: s.clone() })
            .collect::<Vec<_>>()
            .into();

        let mut backoff = Duration::from_secs(1);

        loop {
            // Note the explicit `/` before the query string: without it, some HTTP
            // stacks build a request line with an empty path ("GET ?token=... HTTP/1.1"),
            // which Finnhub's server rejects with 400 Bad Request during the WS handshake.
            let url = format!("wss://ws.finnhub.io/?token={}", config.api_key);

            match tokio_tungstenite::connect_async(url).await {
                Ok((mut ws, _)) => {
                    backoff = Duration::from_secs(1);

                    let mut subscribe_failed = false;
                    for sym in &config.symbols {
                        let sub = serde_json::json!({ "type": "subscribe", "symbol": sym.as_str() });
                        if ws.send(Message::Text(sub.to_string().into())).await.is_err() {
                            subscribe_failed = true;
                            break;
                        }
                    }
                    if subscribe_failed {
                        yield Event::Disconnected(streams.clone(), "failed to send subscribe".into());
                        tokio::time::sleep(backoff).await;
                        continue;
                    }

                    yield Event::Connected(streams.clone());

                    loop {
                        match ws.next().await {
                            Some(Ok(Message::Text(txt))) => {
                                let Ok(raw) = serde_json::from_str::<RawMsg>(&txt) else { continue };
                                if raw.kind != "trade" || raw.data.is_empty() {
                                    continue;
                                }

                                // Finnhub can mix multiple watched symbols in a single message.
                                let mut by_symbol: HashMap<String, Vec<Trade>> = HashMap::new();
                                for rt in raw.data {
                                    let trade = Trade {
                                        time: UnixMs(rt.t),
                                        price: Price::from_f64(rt.p),
                                        qty: Qty(rt.v),
                                        side: None,
                                        conditions: Conditions::from_codes(rt.c),
                                    };
                                    by_symbol.entry(rt.s).or_default().push(trade);
                                }

                                for (sym, trades) in by_symbol {
                                    yield Event::TradesReceived(
                                        StreamKind::Trades { symbol: Symbol::from(sym.as_str()) },
                                        trades.into_boxed_slice(),
                                    );
                                }
                            }
                            Some(Ok(Message::Close(_))) | None => {
                                yield Event::Disconnected(streams.clone(), "connection closed".into());
                                break;
                            }
                            Some(Err(e)) => {
                                yield Event::Disconnected(streams.clone(), e.to_string());
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    yield Event::Disconnected(streams.clone(), e.to_string());
                }
            }

            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(Duration::from_secs(30));
        }
    };

    Box::pin(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_trade_message_shape() {
        let raw = r#"{"type":"trade","data":[{"s":"AAPL","p":310.11,"v":40.0,"t":1785873600000,"c":[]},{"s":"AAPL","p":310.05,"v":160.0,"t":1785873601000,"c":[37]}]}"#;
        let parsed: RawMsg = serde_json::from_str(raw).expect("valid trade message");
        assert_eq!(parsed.kind, "trade");
        assert_eq!(parsed.data.len(), 2);
        assert_eq!(parsed.data[0].s, "AAPL");
        assert_eq!(parsed.data[1].c, vec![37]);
    }

    #[test]
    fn ignores_non_trade_message_types() {
        let raw = r#"{"type":"ping"}"#;
        let parsed: RawMsg = serde_json::from_str(raw).expect("valid ping message");
        assert_eq!(parsed.kind, "ping");
        assert!(parsed.data.is_empty());
    }
}
