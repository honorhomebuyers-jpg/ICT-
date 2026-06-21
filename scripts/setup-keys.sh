#!/usr/bin/env bash
#
# Interactive setup for the backtest data source and API keys.
# Prompts for Alpaca credentials + symbol and writes them to the repo-root .env,
# preserving any existing settings.
#
# Usage:
#   ./scripts/setup-keys.sh
#
set -euo pipefail

# Resolve repo root (this script lives in <root>/scripts).
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="$ROOT/.env"

echo "── Hermes backtest setup ───────────────────────────────────"
echo "Writing to: $ENV_FILE"
echo

# Seed a fresh .env from the example on first run.
if [[ ! -f "$ENV_FILE" && -f "$ROOT/.env.example" ]]; then
  cp "$ROOT/.env.example" "$ENV_FILE"
  echo "Created .env from .env.example"
  echo
fi
touch "$ENV_FILE"

prompt_default() {
  # prompt_default <prompt> <default> -> echoes the answer
  local prompt="$1" def="$2" ans
  read -r -p "$prompt [$def]: " ans
  echo "${ans:-$def}"
}

# Read current value of a key from .env (empty if unset).
current() { grep -E "^$1=" "$ENV_FILE" | tail -n1 | cut -d= -f2- || true; }

DATA_SOURCE="$(prompt_default "Data source (mt5/alpaca)" "$(current DATA_SOURCE || echo alpaca)")"

ALPACA_API_KEY=""
ALPACA_API_SECRET=""
ALPACA_ASSET_CLASS=""
if [[ "$DATA_SOURCE" == "alpaca" ]]; then
  read -r -p "Alpaca API key ID: " ALPACA_API_KEY
  # -s hides the secret as it is typed
  read -r -s -p "Alpaca API secret: " ALPACA_API_SECRET; echo
  ALPACA_ASSET_CLASS="$(prompt_default "Asset class (stock/crypto)" "$(current ALPACA_ASSET_CLASS || echo stock)")"
fi

DEF_SYMBOL="$(current SYMBOL || echo AAPL)"
SYMBOL="$(prompt_default "Symbol" "$DEF_SYMBOL")"

# Upsert a KEY=VALUE pair: drop any existing line for KEY, then append the new one.
upsert() {
  local key="$1" val="$2" tmp
  tmp="$(mktemp)"
  grep -vE "^$key=" "$ENV_FILE" > "$tmp" || true
  printf '%s=%s\n' "$key" "$val" >> "$tmp"
  mv "$tmp" "$ENV_FILE"
}

upsert DATA_SOURCE "$DATA_SOURCE"
upsert SYMBOL      "$SYMBOL"
if [[ "$DATA_SOURCE" == "alpaca" ]]; then
  [[ -n "$ALPACA_API_KEY"     ]] && upsert ALPACA_API_KEY     "$ALPACA_API_KEY"
  [[ -n "$ALPACA_API_SECRET"  ]] && upsert ALPACA_API_SECRET  "$ALPACA_API_SECRET"
  [[ -n "$ALPACA_ASSET_CLASS" ]] && upsert ALPACA_ASSET_CLASS "$ALPACA_ASSET_CLASS"
fi

echo
echo "✓ Saved. Run a backtest with:"
echo "    cargo run --release --bin backtest"
echo
echo "(.env is git-ignored — your keys stay local.)"
