//! M9: Finnhub REST client for institutional/insider positioning data — Form 4
//! insider transactions (~2 business day legal lag), 13F-derived institutional
//! ownership, and STOCK Act congressional trading disclosures (both ~45 day legal
//! lag). None of this is real-time by nature (the lag is a regulatory floor, not a
//! Finnhub limitation), so it's meant to be polled on a slow timer, never per-trade.

use chrono::{Duration, Utc};
use serde::Deserialize;

const BASE_URL: &str = "https://finnhub.io/api/v1";

/// Deliberately holds only a status code / kind and an endpoint label — never the
/// raw `reqwest::Error` or request URL, which would otherwise embed the API key
/// (sent as a header, but easy to leak by accident through a Display impl) into
/// anything that renders this error, including on-screen in the insider panel.
#[derive(Debug, Clone)]
pub enum FetchError {
    Http { endpoint: &'static str, status: u16 },
    Network { endpoint: &'static str },
    Decode { endpoint: &'static str },
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchError::Http { endpoint, status } if *status == 403 => {
                write!(f, "{endpoint}: non disponibile sul piano free (403)")
            }
            FetchError::Http { endpoint, status } => write!(f, "{endpoint}: HTTP {status}"),
            FetchError::Network { endpoint } => write!(f, "{endpoint}: errore di rete"),
            FetchError::Decode { endpoint } => write!(f, "{endpoint}: risposta inattesa"),
        }
    }
}
impl std::error::Error for FetchError {}

#[derive(Debug, Clone)]
pub struct InsiderTransaction {
    pub name: String,
    /// Positive: net buy, negative: net sell (Finnhub's own sign convention).
    pub change: f64,
    pub transaction_date: String,
}

#[derive(Debug, Clone)]
pub struct InstitutionalHolding {
    pub investor_name: String,
    pub share: f64,
}

#[derive(Debug, Clone)]
pub struct CongressionalTrade {
    pub representative: String,
    pub transaction_type: String,
    pub transaction_date: String,
}

#[derive(Clone)]
pub struct FinnhubRestClient {
    http: reqwest::Client,
    api_key: String,
}

impl FinnhubRestClient {
    pub fn new(api_key: String) -> Self {
        Self { http: reqwest::Client::new(), api_key }
    }

    /// Sends the token via the `X-Finnhub-Token` header rather than a `token=`
    /// query parameter, so it never ends up embedded in a URL that could get
    /// logged, displayed in an error message, or show up in a screenshot.
    fn get(&self, path: &str) -> reqwest::RequestBuilder {
        self.http.get(format!("{BASE_URL}{path}")).header("X-Finnhub-Token", &self.api_key)
    }

    pub async fn insider_transactions(&self, symbol: &str) -> Result<Vec<InsiderTransaction>, FetchError> {
        const ENDPOINT: &str = "insider transactions";

        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            data: Vec<RawTxn>,
        }
        #[derive(Deserialize)]
        struct RawTxn {
            name: String,
            change: f64,
            #[serde(rename = "transactionDate")]
            transaction_date: String,
        }

        let resp = self
            .get("/stock/insider-transactions")
            .query(&[("symbol", symbol)])
            .send()
            .await
            .map_err(|_| FetchError::Network { endpoint: ENDPOINT })?;

        if !resp.status().is_success() {
            return Err(FetchError::Http { endpoint: ENDPOINT, status: resp.status().as_u16() });
        }

        let raw: Raw = resp.json().await.map_err(|_| FetchError::Decode { endpoint: ENDPOINT })?;

        Ok(raw
            .data
            .into_iter()
            .map(|t| InsiderTransaction { name: t.name, change: t.change, transaction_date: t.transaction_date })
            .collect())
    }

    pub async fn institutional_ownership(&self, symbol: &str) -> Result<Vec<InstitutionalHolding>, FetchError> {
        const ENDPOINT: &str = "institutional ownership";

        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            data: Vec<RawGroup>,
        }
        #[derive(Deserialize)]
        struct RawGroup {
            #[serde(default)]
            ownership: Vec<RawHolding>,
        }
        #[derive(Deserialize)]
        struct RawHolding {
            name: String,
            share: f64,
        }

        let (from, to) = date_range_days(180);
        let resp = self
            .get("/institutional/ownership")
            .query(&[("symbol", symbol), ("cusip", ""), ("from", from.as_str()), ("to", to.as_str())])
            .send()
            .await
            .map_err(|_| FetchError::Network { endpoint: ENDPOINT })?;

        if !resp.status().is_success() {
            return Err(FetchError::Http { endpoint: ENDPOINT, status: resp.status().as_u16() });
        }

        let raw: Raw = resp.json().await.map_err(|_| FetchError::Decode { endpoint: ENDPOINT })?;

        Ok(raw
            .data
            .into_iter()
            .flat_map(|g| g.ownership)
            .map(|h| InstitutionalHolding { investor_name: h.name, share: h.share })
            .collect())
    }

    pub async fn congressional_trading(&self, symbol: &str) -> Result<Vec<CongressionalTrade>, FetchError> {
        const ENDPOINT: &str = "congressional trading";

        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            data: Vec<RawTrade>,
        }
        #[derive(Deserialize)]
        struct RawTrade {
            name: String,
            #[serde(rename = "transactionType")]
            transaction_type: String,
            #[serde(rename = "transactionDate")]
            transaction_date: String,
        }

        let (from, to) = date_range_days(180);
        let resp = self
            .get("/stock/congressional-trading")
            .query(&[("symbol", symbol), ("from", from.as_str()), ("to", to.as_str())])
            .send()
            .await
            .map_err(|_| FetchError::Network { endpoint: ENDPOINT })?;

        if !resp.status().is_success() {
            return Err(FetchError::Http { endpoint: ENDPOINT, status: resp.status().as_u16() });
        }

        let raw: Raw = resp.json().await.map_err(|_| FetchError::Decode { endpoint: ENDPOINT })?;

        Ok(raw
            .data
            .into_iter()
            .map(|t| CongressionalTrade {
                representative: t.name,
                transaction_type: t.transaction_type,
                transaction_date: t.transaction_date,
            })
            .collect())
    }
}

fn date_range_days(days: i64) -> (String, String) {
    let to = Utc::now().date_naive();
    let from = to - Duration::days(days);
    (from.format("%Y-%m-%d").to_string(), to.format("%Y-%m-%d").to_string())
}
