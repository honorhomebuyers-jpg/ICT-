# Hermes — ICT Algorithmic Trading System

Hermes is a Rust-based algorithmic trading system implementing the **Inner Circle Trader (ICT)** methodology.
It provides a walk-forward backtester and a live MT5 trading engine.

---

## Architecture

```
hermes/
├── bin/
│   ├── backtest/        # Walk-forward backtester with PnL reporting
│   └── hermes/          # Live trading engine
└── crates/
    ├── domain/          # Core types: Candle, Side, Symbol, AccountInfo
    ├── ict/             # ICT signal detection: BOS/CHoCH, OTE, FVG, OB, PD arrays
    ├── risk/            # Fixed-fractional position sizing
    ├── mt5-client/      # MT5 bridge HTTP client
    ├── alpaca-client/   # Alpaca Market Data HTTP client (backtest data source)
    └── engine/          # Orchestrates ICT analysis → risk → order execution
```

The backtester can source historical candles from either the **MT5 bridge**
(default) or the **Alpaca Market Data API**, selected by `DATA_SOURCE`.

---

## ICT Strategy

Hermes analyzes a rolling window of M15 candles using a three-path signal cascade:

**OTE (Primary) → FVG (Fallback A) → OB (Fallback B)**

All paths require BOS/CHoCH confirmation + EMA trend alignment (H1 + optionally H4).

| Concept | Description |
|---------|-------------|
| **BOS / CHoCH** | Break of Structure / Change of Character — confirms directional bias |
| **OTE** | Optimal Trade Entry — 61.8–78.6% Fibonacci retracement of last impulse swing |
| **FVG** | Fair Value Gap — last close inside the most recent unmitigated imbalance zone |
| **Order Block** | Last opposite candle before an impulse; entry when price re-enters OB range |
| **PD Array** | Premium/Discount zones — SL at range_low (Long) or range_high (Short) |
| **Liquidity Sweep** | Tracked as optional confluence flag (no longer required for entry) |

---

## Quick Start

```bash
# 1. Configure environment
cp .env.example .env
# edit .env: set MT5_BASE_URL and SYMBOL

# 2. Run backtest (MT5 data — default)
cargo run --release --bin backtest

# 3. Run live (set SYMBOLS= in .env)
cargo run --release --bin hermes
```

### Backtest with Alpaca data

No MT5 bridge required — fetch candles straight from Alpaca. The fastest way to
enter your keys is the interactive setup command:

```bash
./scripts/setup-keys.sh      # prompts for keys + symbol, writes them to .env
cargo run --release --bin backtest
```

Or configure `.env` by hand:

```bash
# .env:
DATA_SOURCE=alpaca
ALPACA_API_KEY=your_key            # paper keys work for market data
ALPACA_API_SECRET=your_secret
ALPACA_ASSET_CLASS=stock           # or: crypto
SYMBOL=AAPL                        # stock: AAPL  |  crypto: BTC/USD
TIMEFRAME=M15
BACKTEST_CANDLES=5000

cargo run --release --bin backtest
```

### Run a backtest from the web UI

The **Backtest** page in the web dashboard (`web/`) has an interactive runner:
choose a data source, **select a symbol** (or enter a custom one), pick the
timeframe, optionally paste Alpaca keys, and hit **Run Backtest** to execute the
strategy and see live metrics.

```bash
cd web && npm install && npm run dev   # http://localhost:3000/backtest
```

The runner shells out to the `backtest` binary. Set `BACKTEST_BIN` to a prebuilt
binary path to skip `cargo run` on each request.

Alpaca options: `ALPACA_FEED` (`iex` free / `sip` paid, stocks only),
`ALPACA_CRYPTO_LOC` (crypto only), `ALPACA_DIGITS` (price precision), and
`ALPACA_DATA_URL` (base URL override). See [`.env.example`](.env.example).

---

## Backtest Results (XAUUSDm, M15, 50k bars, $5,000 start, 1% risk)

| Metric | Value |
|--------|-------|
| Trades | 148 |
| Win Rate | 54.1% |
| **Profit Factor** | **3.90** |
| **Net Return** | **+209%** |
| Max Drawdown | -$476 |
| Final Balance | $15,433 |

Config: `MIN_RR=2.5`, `DOW_FILTER=true`, `TREND_H4_CONFIRM=true`, `PARTIAL_TP_1R=true`, `PARTIAL_TP_2R=true`, `TRAILING_2R=false`

---

## Configuration

See [`.env.example`](.env.example) for all options with comments.

Key variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `MT5_BASE_URL` | required | MT5 bridge URL |
| `SYMBOL` / `SYMBOLS` | required | Single symbol (backtest) / comma-separated (live) |
| `TIMEFRAME` | required | `M15` recommended |
| `CANDLE_COUNT` | required | Rolling window size (200 for M15) |
| `RISK_PCT` | `0.01` | Risk per trade (1%) |
| `MIN_RR` | `2.5` | Minimum reward:risk ratio |
| `KILLZONE_WINDOWS` | `08:00-13:00,15:00-18:00` | NY + London sessions |
| `DOW_FILTER` | `true` | Skip Monday and Friday |
| `TREND_H4_CONFIRM` | `true` | Require H4 EMA direction agreement |
| `PARTIAL_TP_1R` | `true` | Close 50% at 1R |
| `PARTIAL_TP_2R` | `true` | Close 25% of original at 2R |
| `FRIDAY_CLOSE_HOUR` | `21` | Force-close on Friday at this UTC hour |

---

## Requirements

- **Rust** 1.80+ (edition 2024)
- **MT5 bridge** running and accessible at `MT5_BASE_URL` (for live trading, and
  backtesting with `DATA_SOURCE=mt5`)
- **Alpaca API keys** (only for backtesting with `DATA_SOURCE=alpaca`)

---

## CI/CD

Push a semver tag to trigger a GitHub Actions build and release:

```bash
git tag v1.0.0 && git push origin v1.0.0
```

Artifacts: `hermes-linux-x86_64`, `backtest-linux-x86_64`, `hermes-windows-x86_64.exe`, `backtest-windows-x86_64.exe`
