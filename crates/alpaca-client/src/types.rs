use serde::Deserialize;

/// Asset class served by Alpaca Market Data. Selects the endpoint and the
/// shape of the bars response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetClass {
    /// US equities — `/v2/stocks/{symbol}/bars`.
    Stock,
    /// Crypto — `/v1beta3/crypto/{loc}/bars`.
    Crypto,
}

impl std::str::FromStr for AssetClass {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "stock" | "stocks" | "equity" | "us_equity" => Ok(Self::Stock),
            "crypto" | "cryptocurrency"                  => Ok(Self::Crypto),
            other => Err(format!("unknown ALPACA_ASSET_CLASS: {other} (expected 'stock' or 'crypto')")),
        }
    }
}

/// A single OHLCV bar as returned by Alpaca (both stock and crypto share this shape).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RawBar {
    /// RFC3339 timestamp, e.g. "2024-01-03T09:00:00Z".
    pub t: String,
    pub o: f64,
    pub h: f64,
    pub l: f64,
    pub c: f64,
    /// Volume (shares / coins).
    pub v: f64,
    /// Trade count (optional on some feeds).
    #[serde(default)]
    pub n: u64,
}

/// `/v2/stocks/{symbol}/bars` response.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct StockBarsResponse {
    #[serde(default)]
    pub bars: Vec<RawBar>,
    #[serde(default)]
    pub next_page_token: Option<String>,
}

/// `/v1beta3/crypto/{loc}/bars` response — bars are keyed by symbol.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CryptoBarsResponse {
    #[serde(default)]
    pub bars: std::collections::HashMap<String, Vec<RawBar>>,
    #[serde(default)]
    pub next_page_token: Option<String>,
}

/// Error envelope returned by Alpaca on non-2xx responses.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ApiErrorBody {
    pub message: String,
}
