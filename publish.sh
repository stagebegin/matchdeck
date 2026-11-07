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

# 1) 更新賽程／狀態／比分總表（48h 未開賽、20h 已完場）
if [ "${SKIP_FIX:-0}" != "1" ]; then
  node ../fb/fixtures.mjs --out fixtures.json >/dev/null
  AGE=$(( $(date +%s) - $(stat -c %Y fixtures.json) ))
  if [ "${AGE}" -gt 180 ]; then echo "fixtures.json 過舊（${AGE}s），中止發佈"; exit 4; fi
fi

# 2) 重新生成頁面
./build.sh
REMOTE_URL="https://x-access-token:${TOKEN}@github.com/stagebegin/matchdeck.git"

git add -A
if git diff --cached --quiet; then echo "無變更，略過 push"; exit 0; fi

N=$(ls ../fb/preds/*.json 2>/dev/null | wc -l | tr -d ' ')
BOT="matchdeck bot"
LAST_MSG="$(git log -1 --pretty=%s 2>/dev/null || true)"
LAST_AUTH="$(git log -1 --pretty=%an 2>/dev/null || true)"

if [ "${LAST_AUTH}" = "${BOT}" ] && case "${LAST_MSG}" in auto:*) true;; *) false;; esac; then
  # 上一筆也是自動發佈 → 修訂同一筆，避免每場比分變化都留一個 commit
  git -c user.name="${BOT}" -c user.email="bot@matchdeck.invalid" \
    commit -q --amend -m "auto: $(date -u +%Y-%m-%dT%H:%MZ) · ${N} preds"
  echo "pushing (amend)…"
  REMOTE_SHA="$(git ls-remote "${REMOTE_URL}" refs/heads/main 2>/dev/null | cut -f1)"
  git push --force-with-lease=main:"${REMOTE_SHA}" "${REMOTE_URL}" HEAD:main 2>&1 \
    | sed -E "s/${TOKEN//\//\\/}/***/g"
else
  git -c user.name="${BOT}" -c user.email="bot@matchdeck.invalid" \
    commit -q -m "auto: $(date -u +%Y-%m-%dT%H:%MZ) · ${N} preds"
  echo "pushing…"
  git push "${REMOTE_URL}" HEAD:main 2>&1 \
    | sed -E "s/${TOKEN//\//\\/}/***/g"
fi
echo "已發佈 $(date -u +%H:%MZ)"