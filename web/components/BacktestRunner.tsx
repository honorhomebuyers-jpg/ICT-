"use client";

import { useState } from "react";

const PRESETS: Record<"stock" | "crypto", string[]> = {
  stock:  ["AAPL", "MSFT", "NVDA", "TSLA", "AMZN", "GOOGL", "SPY", "QQQ"],
  crypto: ["BTC/USD", "ETH/USD", "SOL/USD", "LTC/USD"],
};

const TIMEFRAMES = ["M5", "M15", "M30", "H1", "H4", "D1"];
const CUSTOM = "__custom__";

interface Metrics {
  trades: number | null;
  winRate: number | null;
  profitFactor: number | null;
  totalPnl: number | null;
  maxDrawdown: number | null;
  returnPct: number | null;
  finalBalance: number | null;
}

interface RunResult {
  ok: true;
  source: string;
  symbol: string;
  timeframe: string;
  metrics: Metrics;
  log: string;
}

export default function BacktestRunner() {
  const [source, setSource]         = useState<"alpaca" | "mt5">("alpaca");
  const [assetClass, setAssetClass] = useState<"stock" | "crypto">("stock");
  const [symbolPick, setSymbolPick] = useState<string>(PRESETS.stock[0]);
  const [customSymbol, setCustom]   = useState<string>("");
  const [timeframe, setTimeframe]   = useState<string>("M15");
  const [candles, setCandles]       = useState<number>(5000);
  const [risk, setRisk]             = useState<number>(1);
  const [apiKey, setApiKey]         = useState<string>("");
  const [apiSecret, setApiSecret]   = useState<string>("");

  const [running, setRunning] = useState(false);
  const [result, setResult]   = useState<RunResult | null>(null);
  const [error, setError]     = useState<string | null>(null);

  const symbol = symbolPick === CUSTOM ? customSymbol.trim() : symbolPick;

  function onAssetClass(next: "stock" | "crypto") {
    setAssetClass(next);
    setSymbolPick(PRESETS[next][0]);
    setCustom("");
  }

  async function run() {
    setError(null);
    setResult(null);
    if (!symbol) { setError("Please choose or enter a symbol."); return; }
    setRunning(true);
    try {
      const res = await fetch("/api/backtest", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          source,
          symbol,
          assetClass,
          timeframe,
          candles,
          risk: risk / 100,
          apiKey: apiKey || undefined,
          apiSecret: apiSecret || undefined,
        }),
      });
      const data = await res.json();
      if (!res.ok) {
        setError(data.detail ? `${data.error}\n${data.detail}` : (data.error ?? `HTTP ${res.status}`));
      } else {
        setResult(data as RunResult);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  }

  return (
    <div className="card-featured p-6 sm:p-8 mb-10">
      <div className="flex items-center gap-2 mb-1">
        <span className="status-pill status-gold">Interactive</span>
      </div>
      <h2 className="text-xl font-semibold tracking-tight-sm text-ink mb-1">Run a Backtest</h2>
      <p className="text-sm text-ink-sub mb-6">
        Pick a symbol and run the ICT strategy live against Alpaca market data. Keys are
        read from your <code className="font-mono text-ink-md">.env</code> unless entered below.
      </p>

      {/* Data source + asset class */}
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 mb-4">
        <div>
          <label className="field-label">Data Source</label>
          <select className="field-select" value={source}
                  onChange={(e) => setSource(e.target.value as "alpaca" | "mt5")}>
            <option value="alpaca">Alpaca</option>
            <option value="mt5">MT5 bridge</option>
          </select>
        </div>
        {source === "alpaca" && (
          <div>
            <label className="field-label">Asset Class</label>
            <select className="field-select" value={assetClass}
                    onChange={(e) => onAssetClass(e.target.value as "stock" | "crypto")}>
              <option value="stock">Stocks</option>
              <option value="crypto">Crypto</option>
            </select>
          </div>
        )}
      </div>

      {/* Symbol selector */}
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 mb-4">
        <div>
          <label className="field-label">Symbol</label>
          <select className="field-select" value={symbolPick}
                  onChange={(e) => setSymbolPick(e.target.value)}>
            {(source === "alpaca" ? PRESETS[assetClass] : ["XAUUSDm", "GBPUSDm", "EURUSDm", "BTCUSDm"]).map((s) => (
              <option key={s} value={s}>{s}</option>
            ))}
            <option value={CUSTOM}>Custom…</option>
          </select>
        </div>
        {symbolPick === CUSTOM && (
          <div>
            <label className="field-label">Custom Symbol</label>
            <input className="field-input" value={customSymbol} placeholder="e.g. AMD or DOGE/USD"
                   onChange={(e) => setCustom(e.target.value)} />
          </div>
        )}
      </div>

      {/* Timeframe / candles / risk */}
      <div className="grid grid-cols-2 sm:grid-cols-3 gap-4 mb-4">
        <div>
          <label className="field-label">Timeframe</label>
          <select className="field-select" value={timeframe}
                  onChange={(e) => setTimeframe(e.target.value)}>
            {TIMEFRAMES.map((t) => <option key={t} value={t}>{t}</option>)}
          </select>
        </div>
        <div>
          <label className="field-label">Candles</label>
          <input className="field-input" type="number" min={300} max={200000} step={500}
                 value={candles} onChange={(e) => setCandles(Number(e.target.value))} />
        </div>
        <div>
          <label className="field-label">Risk %</label>
          <input className="field-input" type="number" min={0.1} max={50} step={0.1}
                 value={risk} onChange={(e) => setRisk(Number(e.target.value))} />
        </div>
      </div>

      {/* Optional API keys */}
      {source === "alpaca" && (
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 mb-6">
          <div>
            <label className="field-label">Alpaca API Key (optional)</label>
            <input className="field-input" value={apiKey} autoComplete="off" placeholder="from .env if blank"
                   onChange={(e) => setApiKey(e.target.value)} />
          </div>
          <div>
            <label className="field-label">Alpaca API Secret (optional)</label>
            <input className="field-input" type="password" value={apiSecret} autoComplete="off"
                   placeholder="from .env if blank" onChange={(e) => setApiSecret(e.target.value)} />
          </div>
        </div>
      )}

      <div className="flex items-center gap-3">
        <button className="btn-primary" onClick={run} disabled={running}>
          {running ? "Running…" : "Run Backtest →"}
        </button>
        {running && <span className="text-xs text-ink-ter">First run compiles the binary — this can take a minute.</span>}
      </div>

      {error && (
        <pre className="mt-5 text-sm text-bear whitespace-pre-wrap font-mono bg-s2 border border-hl rounded-lg p-4">
          {error}
        </pre>
      )}

      {result && <Results result={result} />}
    </div>
  );
}

function Results({ result }: { result: RunResult }) {
  const m = result.metrics;
  const tiles: Array<{ label: string; value: string; tone?: "bull" | "bear" }> = [
    { label: "Trades",        value: m.trades != null ? String(m.trades) : "—" },
    { label: "Win Rate",      value: m.winRate != null ? `${m.winRate.toFixed(1)}%` : "—" },
    { label: "Profit Factor", value: m.profitFactor != null ? m.profitFactor.toFixed(2) : "—",
      tone: m.profitFactor != null ? (m.profitFactor >= 1 ? "bull" : "bear") : undefined },
    { label: "Net Return",    value: m.returnPct != null ? `${m.returnPct >= 0 ? "+" : ""}${m.returnPct.toFixed(1)}%` : "—",
      tone: m.returnPct != null ? (m.returnPct >= 0 ? "bull" : "bear") : undefined },
    { label: "Total PnL",     value: m.totalPnl != null ? `${m.totalPnl >= 0 ? "+" : ""}${m.totalPnl.toFixed(2)}` : "—",
      tone: m.totalPnl != null ? (m.totalPnl >= 0 ? "bull" : "bear") : undefined },
    { label: "Final Balance", value: m.finalBalance != null ? `$${m.finalBalance.toFixed(2)}` : "—" },
  ];

  return (
    <div className="mt-6 pt-6 border-t border-hl">
      <p className="eyebrow mb-4">
        Result · <span className="font-mono text-ink">{result.symbol}</span> {result.timeframe} · {result.source}
      </p>
      <div className="grid grid-cols-2 sm:grid-cols-3 gap-5 mb-6">
        {tiles.map((t) => (
          <div key={t.label}>
            <p className="text-xs text-ink-sub mb-1.5">{t.label}</p>
            <p className={`font-mono text-lg font-semibold ${
              t.tone === "bull" ? "text-bull" : t.tone === "bear" ? "text-bear" : "text-ink"}`}>
              {t.value}
            </p>
          </div>
        ))}
      </div>
      <details>
        <summary className="text-sm text-ink-sub cursor-pointer hover:text-ink">Show run log</summary>
        <pre className="mt-3 text-xs text-ink-md whitespace-pre-wrap font-mono bg-s2 border border-hl rounded-lg p-4 overflow-x-auto">
          {result.log}
        </pre>
      </details>
    </div>
  );
}
