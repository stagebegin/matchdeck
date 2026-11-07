#!/usr/bin/env bash
# 重新生成靜態頁：需要 Rust 工具鏈（rustup）。
# 產物：index.html + data.json（零外部資源，可直接放 Pages）
#
#   ./build.sh                     # 用同目錄 fixtures.json
#   ./build.sh path/to/fixtures.json
#
# fixtures.json 由 ../fb/fixtures.mjs 產生（賽程＋狀態＋比分＋預測）。
set -euo pipefail
export RUSTUP_HOME="${RUSTUP_HOME:-/data/dsh/home/ahd/.rustup}"
export CARGO_HOME="${CARGO_HOME:-/data/dsh/home/ahd/.cargo}"
export PATH="$CARGO_HOME/bin:$PATH"

FIXTURES="${1:-fixtures.json}"
REGISTRY="${2:-../fb/registry.json}"
TARGET=aarch64-unknown-linux-musl
BIN="target/$TARGET/release/matchdeck"
LINKER="$RUSTUP_HOME/toolchains/stable-aarch64-unknown-linux-gnu/lib/rustlib/aarch64-unknown-linux-gnu/bin/rust-lld"

if [ -z "${RUSTFLAGS:-}" ] && [ -x "$LINKER" ]; then
  export RUSTFLAGS="-C linker=$LINKER -C link-self-contained=yes"
fi

cargo build --release --target "$TARGET" >/dev/null
"$BIN" --fixtures "$FIXTURES" --registry "$REGISTRY" --out dist/index.html
cp dist/data.json ./data.json
cp dist/index.html ./index.html
echo "生成完成：index.html ($(wc -c < index.html) bytes)"