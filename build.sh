#!/usr/bin/env bash
# 重新生成靜態頁：需要 Rust 工具鏈（rustup）。
# 產物：index.html + data.json（零外部資源，可直接放 Pages）
set -euo pipefail
PREDS="${1:-../fb/preds}"
REGISTRY="${2:-../fb/registry.json}"
BIN=target/aarch64-unknown-linux-musl/release/matchdeck
if [ ! -x "$BIN" ]; then
  RUSTFLAGS="-C link-self-contained=yes" cargo build --release --target aarch64-unknown-linux-musl
fi
"$BIN" --preds "$PREDS" --registry "$REGISTRY" --out dist/index.html
cp dist/data.json ./data.json
cp dist/index.html ./index.html
echo "生成完成：index.html ($(wc -c < index.html) bytes)"
