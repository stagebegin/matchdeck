#!/usr/bin/env bash
# publish.sh — 重新生成靜態頁 → commit → push（給 cron / sched.mjs 自動呼叫）
# token 來源：環境變數 GH_TOKEN，或同目錄 .gh_token（chmod 600、已 gitignore）
#
# 數據源（方案 A，用戶 m05273「舊外觀+API」）：
#   首選 fbapi /matches?full=1（node api-fetch.mjs → api-fixtures.json，同構 fixtures.json）；
#   API 不可用時 fallback 返本地 fixtures.json（fixtures.mjs 流程照舊跑，作後備＋同步 db 用）。
set -euo pipefail
cd "$(dirname "$0")"

export RUSTUP_HOME=/data/dsh/home/ahd/.rustup
export CARGO_HOME=/data/dsh/home/ahd/.cargo
export PATH="$CARGO_HOME/bin:$PATH"

TOKEN="${GH_TOKEN:-}"
if [ -z "$TOKEN" ] && [ -f .gh_token ]; then TOKEN="$(tr -d ' \r\n' < .gh_token)"; fi
if [ -z "$TOKEN" ]; then echo "缺少 token：寫入 web/.gh_token 或設 GH_TOKEN"; exit 3; fi

# 0) 監視器（fb/sanity.mjs）：掃出唔合理嘅預測，寫 fb/audit.json 俾卡面帶警告
#    advisory：有 fail 都唔阻發佈，但會喺 console 出聲
node ../fb/sanity.mjs >/dev/null 2>&1 || echo "（監視器有 fail，請跑 node fb/sanity.mjs 睇詳情）"

# 1) 更新賽程／狀態／比分總表（48h 未開賽、72h 已完場）
if [ "${SKIP_FIX:-0}" != "1" ]; then
  node ../fb/fixtures.mjs --out fixtures.json >/dev/null
  # 1a) 補「已完場但缺最終比分」：scache.json 一旦補到就永久保留
  node ../fb/scores.mjs >/dev/null 2>&1 || true
  node ../fb/fixtures.mjs --out fixtures.json >/dev/null
  AGE=$(( $(date +%s) - $(stat -c %Y fixtures.json) ))
  if [ "${AGE}" -gt 180 ]; then echo "fixtures.json 過舊（${AGE}s），中止發佈"; exit 4; fi
  # 1b) 同步 SQLite（matches/preds/settle）→ fbapi 直接讀 db
  node ../fb/db.mjs --import >/dev/null
  node ../fb/db.mjs --settle >/dev/null
fi

# 1b2) 守則閘：冇數據就唔可以出數（用戶 m07721「如果真係無數據就無數據，唔好作數據」）
#      掃 preds/，任何預測用咗「唔喺該聯賽歷史」的隊就即刻中止發佈
if [ "${SKIP_GUARD:-0}" != "1" ]; then
  if ! node ../fb/noprior.mjs; then echo "守則閘：有預測缺數據來源，中止發佈（SKIP_GUARD=1 可臨時繞過）"; exit 5; fi
  if ! node ../fb/ahtest.mjs; then echo "亞盤閘：AHD 不變式失敗，中止發佈（SKIP_GUARD=1 可臨時繞過）"; exit 6; fi
  # 1b3) 洩漏閘：重算已完場比賽時，訓練集唔可以包含嗰場自己嘅賽果（否則等於作數）
  if ! node ../fb/leaktest.mjs; then echo "洩漏閘：有預測嘅訓練集包含自己賽果，中止發佈（SKIP_GUARD=1 可臨時繞過）"; exit 7; fi
fi

# 1c) 數據源：首選 API；失敗（server 唔喺等）用本地 fixtures.json
SRC="fixtures.json"
if node api-fetch.mjs >/dev/null 2>&1; then
  SRC="api-fixtures.json"
  echo "數據源：fbapi API"
else
  echo "數據源：本地 fixtures.json（API 不可用，fallback）"
fi

# 2) 重新生成頁面（舊靜態版，霧青玻璃樣）
./build.sh "$SRC"

# 2a) 頁面層守則閘：每張卡都要有 data-id／data-status／data-comp，
#     否則 60 秒自動更新會漏掉佢（ghost 卡曾冇 data-id → 永遠卡喺「未開賽」）
if [ "${SKIP_GUARD:-0}" != "1" ]; then
  if ! node cardtest.mjs; then echo "頁面閘：卡片結構有問題，中止發佈（SKIP_GUARD=1 可臨時繞過）"; exit 8; fi
fi

# 2b) Step 3 附頁：盃賽模擬、報告、國際賽事（冇資料就略過）
if ls ../fb/tournament/cup-*.json >/dev/null 2>&1; then
  node mkcups.mjs >/dev/null || echo "（盃賽頁生成失敗，略過）"
fi
node mkreport.mjs --out report.html >/dev/null 2>&1 || echo "（報告頁生成失敗，略過）"
node mkaudit.mjs >/dev/null 2>&1 || echo "（監視頁生成失敗，略過）"
if ls ../fb/tournament/[0-9]*.json >/dev/null 2>&1; then
  node mktour.mjs >/dev/null 2>&1 || echo "（國際賽事頁生成失敗，略過）"
fi
if ls ../fb/tournament/eu-*.json >/dev/null 2>&1; then
  node mkeuropa.mjs >/dev/null 2>&1 || echo "（歐聯／歐霸頁生成失敗，略過）"
fi
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