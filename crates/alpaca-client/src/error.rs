#[derive(Debug, thiserror::Error)]
pub enum AlpacaError {
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("alpaca api error ({status}): {detail}")]
    Api { status: u16, detail: String },

    #[error("response parse failed: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("invalid configuration: {0}")]
    Config(String),
}
