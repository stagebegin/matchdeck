#!/usr/bin/env bash
# publish.sh — 重新生成靜態頁 → commit → push（給 cron / sched.mjs 自動呼叫）
# token 來源：環境變數 GH_TOKEN，或同目錄 .gh_token（chmod 600、已 gitignore）
set -euo pipefail
cd "$(dirname "$0")"

export RUSTUP_HOME=/data/dsh/home/ahd/.rustup
export CARGO_HOME=/data/dsh/home/ahd/.cargo
export PATH="$CARGO_HOME/bin:$PATH"

TOKEN="${GH_TOKEN:-}"
if [ -z "$TOKEN" ] && [ -f .gh_token ]; then TOKEN="$(tr -d ' \r\n' < .gh_token)"; fi
if [ -z "$TOKEN" ]; then echo "缺少 token：寫入 web/.gh_token 或設 GH_TOKEN"; exit 3; fi

./build.sh
git add -A
if git diff --cached --quiet; then echo "無變更，略過 push"; exit 0; fi

N=$(ls ../fb/preds/*.json 2>/dev/null | wc -l | tr -d ' ')
git -c user.name="matchdeck bot" -c user.email="bot@matchdeck.invalid" \
  commit -q -m "auto: $(date -u +%Y-%m-%dT%H:%MZ) · ${N} preds"

echo "pushing…"
git push "https://x-access-token:${TOKEN}@github.com/stagebegin/matchdeck.git" HEAD:main 2>&1 \
  | sed -E "s/${TOKEN//\//\\/}/***/g"
echo "已發佈 $(date -u +%H:%MZ)"