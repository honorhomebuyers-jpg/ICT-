import { NextResponse } from "next/server";
import { spawn } from "node:child_process";
import path from "node:path";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

// The Rust binary reads all configuration from environment variables, so a run is
// just: build an env, spawn the binary in the repo root, parse its summary block.

const ALLOWED_TF = new Set(["M5", "M15", "M30", "H1", "H4", "D1"]);
const SYMBOL_RE = /^[A-Za-z0-9/._-]{1,20}$/;
const RUN_TIMEOUT_MS = 240_000; // first run may compile; subsequent runs are fast

interface RunBody {
  source?: string;       // "alpaca" | "mt5"
  symbol?: string;
  assetClass?: string;   // "stock" | "crypto"
  timeframe?: string;
  candles?: number;
  risk?: number;
  balance?: number;
  apiKey?: string;       // optional override; falls back to server .env
  apiSecret?: string;
  feed?: string;         // "iex" | "sip"
}

function parseSummary(out: string) {
  const num = (re: RegExp): number | null => {
    const m = out.match(re);
    if (!m) return null;
    const v = parseFloat(m[1].replace(/[+,]/g, ""));
    return Number.isFinite(v) ? v : null;
  };
  return {
    trades:       num(/Trades\s*:\s*([\d,]+)/),
    winRate:      num(/Win\s*:\s*\d+\s*\(([\d.]+)%\)/),
    profitFactor: num(/Profit factor\s*:\s*([\d.]+)/),
    totalPnl:     num(/Total PnL\s*:\s*([+\-]?[\d.,]+)/),
    maxDrawdown:  num(/Max Drawdown\s*:\s*([+\-]?[\d.,]+)/),
    returnPct:    num(/Return\s*:\s*([+\-]?[\d.]+)%/),
    finalBalance: num(/Final Balance\s*:\s*([\d.,]+)/),
  };
}

export async function POST(req: Request) {
  let body: RunBody;
  try {
    body = await req.json();
  } catch {
    return NextResponse.json({ error: "invalid JSON body" }, { status: 400 });
  }

  // ── validate ───────────────────────────────────────────────────────────────
  const source = (body.source ?? "alpaca").toLowerCase();
  if (source !== "alpaca" && source !== "mt5") {
    return NextResponse.json({ error: "source must be 'alpaca' or 'mt5'" }, { status: 400 });
  }
  const symbol = (body.symbol ?? "").trim();
  if (!SYMBOL_RE.test(symbol)) {
    return NextResponse.json({ error: "invalid symbol" }, { status: 400 });
  }
  const timeframe = (body.timeframe ?? "M15").toUpperCase();
  if (!ALLOWED_TF.has(timeframe)) {
    return NextResponse.json({ error: `timeframe must be one of ${[...ALLOWED_TF].join(", ")}` }, { status: 400 });
  }
  const assetClass = (body.assetClass ?? "stock").toLowerCase();
  if (assetClass !== "stock" && assetClass !== "crypto") {
    return NextResponse.json({ error: "assetClass must be 'stock' or 'crypto'" }, { status: 400 });
  }
  const candles = Math.min(Math.max(Math.round(body.candles ?? 5000), 300), 200_000);
  const risk    = Math.min(Math.max(body.risk ?? 0.01, 0.0001), 0.5);
  const balance = Math.min(Math.max(body.balance ?? 5000, 1), 100_000_000);
  const feed    = (body.feed ?? "iex").toLowerCase() === "sip" ? "sip" : "iex";

  // ── build child env ──────────────────────────────────────────────────────────
  const repoRoot = path.resolve(process.cwd(), "..");
  const env: NodeJS.ProcessEnv = {
    ...process.env,
    DATA_SOURCE:      source,
    SYMBOL:           symbol,
    TIMEFRAME:        timeframe,
    CANDLE_COUNT:     String(process.env.CANDLE_COUNT ?? 200),
    RISK_PCT:         String(risk),
    BACKTEST_CANDLES: String(candles),
    BACKTEST_BALANCE: String(balance),
  };
  if (source === "alpaca") {
    env.ALPACA_ASSET_CLASS = assetClass;
    env.ALPACA_FEED = feed;
    // UI-entered keys take precedence; otherwise the binary loads them from the
    // repo-root .env itself (via dotenvy), so we don't require them here.
    if (body.apiKey?.trim())    env.ALPACA_API_KEY = body.apiKey.trim();
    if (body.apiSecret?.trim()) env.ALPACA_API_SECRET = body.apiSecret.trim();
  }

  // ── command: prefer a prebuilt binary, fall back to `cargo run` ───────────────
  const binOverride = process.env.BACKTEST_BIN;
  const [cmd, args] = binOverride
    ? [binOverride, [] as string[]]
    : ["cargo", ["run", "--release", "--quiet", "--bin", "backtest"]];

  try {
    const { code, stdout, stderr } = await runProcess(cmd, args, repoRoot, env);
    if (code !== 0) {
      const detail = (stderr || stdout).trim().split("\n").slice(-8).join("\n");
      return NextResponse.json({ error: `backtest exited with code ${code}`, detail }, { status: 500 });
    }
    const metrics = parseSummary(stdout);
    // Return the last slice of stdout (summary + recent trades) for display.
    const tail = stdout.trim().split("\n").slice(-40).join("\n");
    return NextResponse.json({ ok: true, source, symbol, timeframe, metrics, log: tail });
  } catch (e) {
    return NextResponse.json({ error: String(e) }, { status: 500 });
  }
}

function runProcess(
  cmd: string,
  args: string[],
  cwd: string,
  env: NodeJS.ProcessEnv,
): Promise<{ code: number; stdout: string; stderr: string }> {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { cwd, env, shell: false });
    let stdout = "";
    let stderr = "";
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      reject(new Error(`backtest timed out after ${RUN_TIMEOUT_MS / 1000}s`));
    }, RUN_TIMEOUT_MS);

    child.stdout.on("data", (d) => { stdout += d.toString(); });
    child.stderr.on("data", (d) => { stderr += d.toString(); });
    child.on("error", (err) => {
      clearTimeout(timer);
      reject(
        (err as NodeJS.ErrnoException).code === "ENOENT"
          ? new Error(`'${cmd}' not found — install Rust/cargo or set BACKTEST_BIN to the compiled binary`)
          : err,
      );
    });
    child.on("close", (code) => {
      clearTimeout(timer);
      resolve({ code: code ?? -1, stdout, stderr });
    });
  });
}
