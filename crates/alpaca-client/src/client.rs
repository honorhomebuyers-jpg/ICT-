use chrono::{DateTime, Utc};
use domain::{Candle, Symbol, Timeframe};
use rust_decimal::Decimal;

use crate::error::AlpacaError;
use crate::types::{ApiErrorBody, AssetClass, CryptoBarsResponse, RawBar, StockBarsResponse};

/// Default Alpaca Market Data base URL.
pub const DEFAULT_DATA_URL: &str = "https://data.alpaca.markets";

/// Maximum bars Alpaca returns per page.
const PAGE_LIMIT: u32 = 10_000;

/// Configuration for the Alpaca historical-data client.
#[derive(Debug, Clone)]
pub struct AlpacaConfig {
    pub base_url:    String,
    pub api_key:     String,
    pub api_secret:  String,
    pub asset_class: AssetClass,
    /// Data feed for stocks (`iex` free / `sip` paid). Ignored for crypto.
    pub feed:        String,
    /// Crypto location (e.g. "us"). Ignored for stocks.
    pub crypto_loc:  String,
    /// Price precision used to synthesize the `Symbol` (digits + derived point).
    pub digits:      u8,
}

pub struct AlpacaClient {
    cfg:  AlpacaConfig,
    http: reqwest::Client,
}

impl AlpacaClient {
    pub fn new(cfg: AlpacaConfig) -> Self {
        Self { cfg, http: reqwest::Client::new() }
    }

    /// Synthesize a `Symbol` for the backtester. Alpaca has no MT5-style symbol
    /// metadata, so we derive sensible defaults: 1 unit = 1 share/coin, USD-quoted,
    /// no broker spread (Alpaca bars are trade-based, not bid/ask).
    pub fn synth_symbol(&self, name: &str) -> Symbol {
        let digits = self.cfg.digits;
        let point  = Decimal::new(1, digits as u32); // 10^-digits
        let category = match self.cfg.asset_class {
            AssetClass::Stock  => "Stocks",
            AssetClass::Crypto => "Crypto",
        };
        Symbol {
            name:                name.to_string(),
            description:         format!("Alpaca {category} {name}"),
            digits,
            point,
            bid:                 Decimal::ZERO,
            ask:                 Decimal::ZERO,
            spread:              0,
            spread_float:        false,
            volume_min:          Decimal::ONE,
            volume_max:          Decimal::from(1_000_000u32),
            volume_step:         Decimal::ONE,
            trade_contract_size: Decimal::ONE,
            currency_base:       name.to_string(),
            currency_profit:     "USD".to_string(),
            category:            category.to_string(),
        }
    }

    /// Fetch the most recent `count` bars for `symbol` at `timeframe`, ascending by time.
    ///
    /// Alpaca paginates forward from a `start` timestamp, so we estimate a start far
    /// enough back to cover `count` bars (allowing for nights/weekends/holidays),
    /// page through to `now`, then keep the last `count`.
    pub async fn recent_bars(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        count: u32,
    ) -> Result<Vec<Candle>, AlpacaError> {
        let end   = Utc::now();
        let start = end - estimated_lookback(timeframe, count);
        let mut bars = self.bars(symbol, timeframe, start, end).await?;
        let n = count as usize;
        if bars.len() > n {
            bars.drain(0..bars.len() - n);
        }
        Ok(bars)
    }

    /// Fetch all bars in `[start, end]` for `symbol` at `timeframe`, ascending by time,
    /// following pagination tokens until the range is exhausted.
    pub async fn bars(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        ) -> Result<Vec<Candle>, AlpacaError> {
        let tf = alpaca_timeframe(timeframe);
        let start_s = start.to_rfc3339();
        let end_s   = end.to_rfc3339();

        let mut out: Vec<Candle> = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let raw = match self.cfg.asset_class {
                AssetClass::Stock  => self.fetch_stock_page(symbol, &tf, &start_s, &end_s, page_token.as_deref()).await?,
                AssetClass::Crypto => self.fetch_crypto_page(symbol, &tf, &start_s, &end_s, page_token.as_deref()).await?,
            };
            let (raw_bars, next) = raw;
            for b in raw_bars {
                out.push(raw_to_candle(b)?);
            }
            match next {
                Some(tok) => page_token = Some(tok),
                None => break,
            }
        }

        tracing::debug!(symbol, bars = out.len(), "alpaca bars fetched");
        Ok(out)
    }

    async fn fetch_stock_page(
        &self,
        symbol: &str,
        tf: &str,
        start: &str,
        end: &str,
        page_token: Option<&str>,
    ) -> Result<(Vec<RawBar>, Option<String>), AlpacaError> {
        let url = format!("{}/v2/stocks/{symbol}/bars", self.cfg.base_url);
        let limit = PAGE_LIMIT.to_string();
        let mut query: Vec<(&str, String)> = vec![
            ("timeframe",  tf.to_string()),
            ("start",      start.to_string()),
            ("end",        end.to_string()),
            ("limit",      limit),
            ("adjustment", "all".to_string()),
            ("feed",       self.cfg.feed.clone()),
            ("sort",       "asc".to_string()),
        ];
        if let Some(tok) = page_token {
            query.push(("page_token", tok.to_string()));
        }
        let text = self.get_authed(&url, &query).await?;
        let resp: StockBarsResponse = serde_json::from_str(&text)?;
        Ok((resp.bars, resp.next_page_token))
    }

    async fn fetch_crypto_page(
        &self,
        symbol: &str,
        tf: &str,
        start: &str,
        end: &str,
        page_token: Option<&str>,
    ) -> Result<(Vec<RawBar>, Option<String>), AlpacaError> {
        let url = format!("{}/v1beta3/crypto/{}/bars", self.cfg.base_url, self.cfg.crypto_loc);
        let limit = PAGE_LIMIT.to_string();
        let mut query: Vec<(&str, String)> = vec![
            ("symbols",   symbol.to_string()),
            ("timeframe", tf.to_string()),
            ("start",     start.to_string()),
            ("end",       end.to_string()),
            ("limit",     limit),
            ("sort",      "asc".to_string()),
        ];
        if let Some(tok) = page_token {
            query.push(("page_token", tok.to_string()));
        }
        let text = self.get_authed(&url, &query).await?;
        let mut resp: CryptoBarsResponse = serde_json::from_str(&text)?;
        // Crypto bars are keyed by symbol; pull the requested one (case-insensitive).
        let bars = resp.bars.remove(symbol)
            .or_else(|| {
                let key = resp.bars.keys().find(|k| k.eq_ignore_ascii_case(symbol)).cloned();
                key.and_then(|k| resp.bars.remove(&k))
            })
            .unwrap_or_default();
        Ok((bars, resp.next_page_token))
    }

    async fn get_authed(&self, url: &str, query: &[(&str, String)]) -> Result<String, AlpacaError> {
        let resp = self.http
            .get(url)
            .header("APCA-API-KEY-ID", &self.cfg.api_key)
            .header("APCA-API-SECRET-KEY", &self.cfg.api_secret)
            .query(query)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let code = status.as_u16();
            let body = resp.text().await.unwrap_or_default();
            let detail = serde_json::from_str::<ApiErrorBody>(&body)
                .map(|e| e.message)
                .unwrap_or(body);
            return Err(AlpacaError::Api { status: code, detail });
        }
        Ok(resp.text().await?)
    }
}

/// Convert an Alpaca timeframe into the MT5-style domain `Timeframe`'s Alpaca string.
/// Alpaca accepts `<n>Min`, `<n>Hour`, `1Day`, `1Week`, `1Month`.
fn alpaca_timeframe(tf: Timeframe) -> String {
    match tf {
        Timeframe::M1  => "1Min".into(),
        Timeframe::M2  => "2Min".into(),
        Timeframe::M3  => "3Min".into(),
        Timeframe::M4  => "4Min".into(),
        Timeframe::M5  => "5Min".into(),
        Timeframe::M6  => "6Min".into(),
        Timeframe::M10 => "10Min".into(),
        Timeframe::M12 => "12Min".into(),
        Timeframe::M15 => "15Min".into(),
        Timeframe::M20 => "20Min".into(),
        Timeframe::M30 => "30Min".into(),
        Timeframe::H1  => "1Hour".into(),
        Timeframe::H2  => "2Hour".into(),
        Timeframe::H3  => "3Hour".into(),
        Timeframe::H4  => "4Hour".into(),
        Timeframe::H6  => "6Hour".into(),
        Timeframe::H8  => "8Hour".into(),
        Timeframe::H12 => "12Hour".into(),
        Timeframe::D1  => "1Day".into(),
        Timeframe::W1  => "1Week".into(),
        Timeframe::Mn1 => "1Month".into(),
    }
}

/// Whole minutes covered by one bar of `tf`.
fn tf_minutes(tf: Timeframe) -> i64 {
    match tf {
        Timeframe::M1  => 1,
        Timeframe::M2  => 2,
        Timeframe::M3  => 3,
        Timeframe::M4  => 4,
        Timeframe::M5  => 5,
        Timeframe::M6  => 6,
        Timeframe::M10 => 10,
        Timeframe::M12 => 12,
        Timeframe::M15 => 15,
        Timeframe::M20 => 20,
        Timeframe::M30 => 30,
        Timeframe::H1  => 60,
        Timeframe::H2  => 120,
        Timeframe::H3  => 180,
        Timeframe::H4  => 240,
        Timeframe::H6  => 360,
        Timeframe::H8  => 480,
        Timeframe::H12 => 720,
        Timeframe::D1  => 1_440,
        Timeframe::W1  => 10_080,
        Timeframe::Mn1 => 43_200,
    }
}

/// Estimate how far back to look to collect `count` bars. Intraday bars only print
/// during market hours, so we pad the raw calendar span generously; over-fetching is
/// harmless because `recent_bars` keeps only the last `count`.
fn estimated_lookback(tf: Timeframe, count: u32) -> chrono::Duration {
    let raw_minutes = tf_minutes(tf).saturating_mul(count.max(1) as i64);
    // Daily and above print every calendar period for crypto and most trading days
    // for stocks; intraday needs a larger multiplier to cover nights/weekends.
    let padded = if tf_minutes(tf) >= 1_440 {
        raw_minutes.saturating_mul(2)
    } else {
        raw_minutes.saturating_mul(4)
    };
    chrono::Duration::minutes(padded)
}

fn raw_to_candle(b: RawBar) -> Result<Candle, AlpacaError> {
    let time = DateTime::parse_from_rfc3339(&b.t)
        .map_err(|e| AlpacaError::Config(format!("bad bar timestamp '{}': {e}", b.t)))?
        .with_timezone(&Utc);
    let dec = |v: f64, field: &str| -> Result<Decimal, AlpacaError> {
        Decimal::try_from(v).map_err(|e| AlpacaError::Config(format!("bad {field}={v}: {e}")))
    };
    Ok(Candle {
        time,
        open:        dec(b.o, "open")?,
        high:        dec(b.h, "high")?,
        low:         dec(b.l, "low")?,
        close:       dec(b.c, "close")?,
        tick_volume: b.n,
        spread:      0,
        real_volume: b.v.max(0.0) as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CryptoBarsResponse, StockBarsResponse};

    #[test]
    fn timeframe_mapping_spot_check() {
        assert_eq!(alpaca_timeframe(Timeframe::M15), "15Min");
        assert_eq!(alpaca_timeframe(Timeframe::H4),  "4Hour");
        assert_eq!(alpaca_timeframe(Timeframe::D1),  "1Day");
    }

    #[test]
    fn parse_stock_bars() {
        let raw = r#"{"bars":[{"t":"2024-01-03T09:00:00Z","o":185.0,"h":186.5,"l":184.2,"c":185.9,"v":120345,"n":982,"vw":185.4}],"symbol":"AAPL","next_page_token":"abc"}"#;
        let r: StockBarsResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(r.bars.len(), 1);
        assert_eq!(r.next_page_token.as_deref(), Some("abc"));
        let c = raw_to_candle(r.bars.into_iter().next().unwrap()).unwrap();
        assert_eq!(c.close, "185.9".parse::<Decimal>().unwrap());
        assert_eq!(c.tick_volume, 982);
        assert_eq!(c.real_volume, 120345);
    }

    #[test]
    fn parse_crypto_bars_keyed_by_symbol() {
        let raw = r#"{"bars":{"BTC/USD":[{"t":"2024-01-03T09:00:00Z","o":42000.0,"h":42500.0,"l":41800.0,"c":42250.0,"v":12.5,"n":340,"vw":42100.0}]},"next_page_token":null}"#;
        let r: CryptoBarsResponse = serde_json::from_str(raw).unwrap();
        assert!(r.bars.contains_key("BTC/USD"));
        assert!(r.next_page_token.is_none());
    }

    #[test]
    fn empty_bars_field_defaults() {
        let r: StockBarsResponse = serde_json::from_str(r#"{"symbol":"AAPL","next_page_token":null}"#).unwrap();
        assert!(r.bars.is_empty());
        assert!(r.next_page_token.is_none());
    }

    #[test]
    fn lookback_pads_intraday_more_than_daily() {
        // 100 x 15Min raw span = 1500 min; intraday pads x4.
        assert_eq!(estimated_lookback(Timeframe::M15, 100), chrono::Duration::minutes(1500 * 4));
        // 100 x 1Day raw span = 144000 min; daily pads x2.
        assert_eq!(estimated_lookback(Timeframe::D1, 100), chrono::Duration::minutes(144_000 * 2));
    }

    #[test]
    fn synth_symbol_derives_point_from_digits() {
        let cfg = AlpacaConfig {
            base_url: DEFAULT_DATA_URL.into(),
            api_key: "k".into(),
            api_secret: "s".into(),
            asset_class: AssetClass::Stock,
            feed: "iex".into(),
            crypto_loc: "us".into(),
            digits: 2,
        };
        let s = AlpacaClient::new(cfg).synth_symbol("AAPL");
        assert_eq!(s.digits, 2);
        assert_eq!(s.point, "0.01".parse::<Decimal>().unwrap());
        assert_eq!(s.volume_step, Decimal::ONE);
        assert_eq!(s.currency_profit, "USD");
    }
}
