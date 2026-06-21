#!/usr/bin/env bash
#
# Installs the Rust toolchain (cargo + rustc) needed to build the backtest.
#
# Run it by name — no URL to type, nothing for copy-paste to mangle:
#
#     bash scripts/install-rust.sh
#
set -euo pipefail

echo "── Looking for an existing Rust toolchain ──────────────────"

# cargo may already be installed somewhere not on PATH (common in Codespaces
# and Docker images). Check the usual spots before downloading anything.
for dir in "$HOME/.cargo/bin" /usr/local/cargo/bin /opt/cargo/bin; do
  if [ -x "$dir/cargo" ]; then
    export PATH="$dir:$PATH"
    echo "Found cargo: $dir/cargo"
    "$dir/cargo" --version
    echo
    echo "✓ Rust is already installed — it just was not on your PATH."
    echo "  Make it permanent for new terminals:"
    echo "      echo 'export PATH=\"$dir:\$PATH\"' >> ~/.bashrc"
    echo
    echo "Then build:  cargo run --release --bin backtest"
    exit 0
  fi
done

echo "No cargo found. Installing Rust via rustup (~200MB download)…"
echo

# The rustup installer URL lives HERE, inside the file, so copy-paste in your
# terminal never touches it. `-y` accepts defaults and adds cargo to your PATH
# for future shells.
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Load cargo into THIS shell right away.
# shellcheck disable=SC1091
. "$HOME/.cargo/env"

echo
cargo --version
echo
echo "✓ Rust installed."
echo "  This terminal can use cargo now. New terminals pick it up automatically"
echo "  (or run:  . \"\$HOME/.cargo/env\")."
echo
echo "Next:  cargo run --release --bin backtest"
