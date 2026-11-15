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

# 發佈耗時分解（用戶 m01248 問點樣快啲）：時間 / 階段 / 累計ms / 本段ms
PT0=$(date +%s%3N); PTLAST=$PT0
t() { local now; now=$(date +%s%3N); printf '%s\t%s\t%s\t%s\n' "$(date -u +%H:%M:%SZ)" "$1" "$((now-PT0))" "$((now-PTLAST))" >> ../fb/publish-timing.log; PTLAST=$now; }
t "start"

# 0) 監視器（fb/sanity.mjs）：掃出唔合理嘅預測，寫 fb/audit.json 俾卡面帶警告
#    advisory：有 fail 都唔阻發佈，但會喺 console 出聲
node ../fb/sanity.mjs >/dev/null 2>&1 || echo "（監視器有 fail，請跑 node fb/sanity.mjs 睇詳情）"
t "0. sanity"

# 1) 更新賽程／狀態／比分總表（48h 未開賽、72h 已完場）
if [ "${SKIP_FIX:-0}" != "1" ]; then
  # 1-0) 重複場次自動清理：馬會改期會換 id，舊 id 會令同一場出兩張卡（用戶 m19439）。
  #      呢一步係 idempotent：冇重複就乜都唔做；有就留最新／有比分／有預測嗰個。
  node ../fb/dedupe.mjs --apply 2>/dev/null | grep -v '^重複場次：0' || true
  node ../fb/fixtures.mjs --out fixtures.json >/dev/null
  # 1a)
t "1a. dedupe+fixtures" 補「已完場但缺最終比分」：scache.json 一旦補到就永久保留
  #     只有真係補到新比分才需要重跑一次 fixtures（每次 30 秒；快線每 90 秒發佈一次負擔唔起）
  SC0="$(md5sum ../fb/scache.json 2>/dev/null | cut -d' ' -f1)"
  node ../fb/scores.mjs >/dev/null 2>&1 || true
  t "1a2. scores"
  SC1="$(md5sum ../fb/scache.json 2>/dev/null | cut -d' ' -f1)"
  if [ "${SC1}" != "${SC0}" ]; then node ../fb/fixtures.mjs --out fixtures.json >/dev/null; fi
  t "1a3. refixtures"
  AGE=$(( $(date +%s) - $(stat -c %Y fixtures.json) ))
  if [ "${AGE}" -gt 180 ]; then echo "fixtures.json 過舊（${AGE}s），中止發佈"; exit 4; fi
  # 1b) 同步 SQLite（matches/preds/settle）→ fbapi 直接讀 db
  node ../fb/db.mjs --import >/dev/null
  node ../fb/db.mjs --settle >/dev/null
  t "1b. db import/settle"
fi

# 1b2) 守則閘：冇數據就唔可以出數（用戶 m07721「如果真係無數據就無數據，唔好作數據」）
#      掃 preds/，任何預測用咗「唔喺該聯賽歷史」的隊就即刻中止發佈
if [ "${SKIP_GUARD:-0}" != "1" ]; then
  if ! node ../fb/noprior.mjs; then echo "守則閘：有預測缺數據來源，中止發佈（SKIP_GUARD=1 可臨時繞過）"; exit 5; fi
  t "gate. noprior"
  if ! node ../fb/ahtest.mjs; then echo "亞盤閘：AHD 不變式失敗，中止發佈（SKIP_GUARD=1 可臨時繞過）"; exit 6; fi
  t "gate. ahtest"
  # 1b3) 洩漏閘：重算已完場比賽時，訓練集唔可以包含嗰場自己嘅賽果（否則等於作數）
  if ! node ../fb/leaktest.mjs; then echo "洩漏閘：有預測嘅訓練集包含自己賽果，中止發佈（SKIP_GUARD=1 可臨時繞過）"; exit 7; fi
  t "gate. leaktest"
  if ! node ../fb/memocheck.mjs; then echo "Memo 閘：memo 標籤唔合五格規格，中止發佈（SKIP_GUARD=1 可臨時繞過）"; exit 9; fi
  t "gate. memocheck"
  if ! node ../fb/memoverify.mjs; then echo "Memo 對帳閘：memo 嘅模型盤／初終盤／標籤同來源數據唔一致，中止發佈（SKIP_GUARD=1 可臨時繞過）"; exit 11; fi
  t "gate. memoverify"
fi

# 1c) 數據源：首選 API；失敗（server 唔喺等）用本地 fixtures.json
SRC="fixtures.json"
if node api-fetch.mjs >/dev/null 2>&1; then
  SRC="api-fixtures.json"
  echo "數據源：fbapi API"
else
  echo "數據源：本地 fixtures.json（API 不可用，fallback）"
fi
t "1c. api-fetch"

# 1d) 標籤補齊：無論用 API 定本地檔，都用 data.db 補 memo tags（API 只帶到部分場次）
node tagmerge.mjs "$SRC" || true
t "1d. tagmerge"

# 1e) 重複卡閘：同一場比賽唔可以出兩張卡（馬會改期換 id 會造成重複，用戶 m19439）
if [ "${SKIP_GUARD:-0}" != "1" ]; then
  if ! node dupgate.mjs "$SRC"; then echo "重複卡閘：同一場比賽出現多過一張卡，中止發佈（node ../fb/dedupe.mjs --apply 清理；SKIP_GUARD=1 可臨時繞過）"; exit 13; fi
  t "1e. dupgate"
fi

# 2) 重新生成頁面（舊靜態版，霧青玻璃樣）
./build.sh "$SRC"
t "2. build(rust)"

# 2a) 頁面層守則閘：每張卡都要有 data-id／data-status／data-comp，
#     否則 60 秒自動更新會漏掉佢（ghost 卡曾冇 data-id → 永遠卡喺「未開賽」）
if [ "${SKIP_GUARD:-0}" != "1" ]; then
  if ! node cardtest.mjs; then echo "頁面閘：卡片結構有問題，中止發佈（SKIP_GUARD=1 可臨時繞過）"; exit 8; fi
    t "gate. cardtest"
  # 2a2) 搬卡邏輯閘：直接跑 src/main.rs 裏面真正嘅 placeCard／ensureGroup，
  #      驗「狀態變咗會唔會搬去正確分頁＋日期組排序」（用戶 m17055：比賽中分頁冇進行中場次）
  if ! node movetest.mjs; then echo "搬卡閘：狀態轉換唔會搬去正確分頁，中止發佈（SKIP_GUARD=1 可臨時繞過）"; exit 10; fi
    t "gate. movetest"
  # 2a2b) 自動修：模型預測重算過（run.mjs 出咗新 ahd.line）但 memo 仲記住舊模型公平盤，
  #       由 memo.data（初／終盤）＋ preds 現值重算五格並覆寫，唔需要重抓時間線。
  #       唔理成敗——真係修唔到就交返下面嘅稽核閘報錯（用戶 m19901：發佈靜靜死咗成晚）
  node ../fb/tagrelabel.mjs > /tmp/tagrelabel.out 2>&1 || true
  if grep -q "已寫 [1-9]" /tmp/tagrelabel.out; then echo "標籤自動修：$(grep -o '要改 [0-9]*、已寫 [0-9]*' /tmp/tagrelabel.out | head -1)"; fi
    t "2a2b. tagrelabel"
  # 2a3) 自動標籤獨立稽核：唔用 automemo 嘅程式碼，自己由 memo.data 重算五格再比對
  if ! node ../fb/tagaudit.mjs; then echo "標籤稽核閘：自動 hashtag 同數據重算結果唔一致，中止發佈（SKIP_GUARD=1 可臨時繞過）"; exit 12; fi
    t "gate. tagaudit"
fi

# 2b) Step 3 附頁：盃賽模擬、報告、國際賽事（冇資料就略過）
#     FASTPUB=1（live 快線）跳過：呢啲頁同比分無關，每 90 秒重建一次純粹浪費時間。
if [ "${FASTPUB:-0}" != "1" ]; then

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
fi

REMOTE_URL="https://x-access-token:${TOKEN}@github.com/stagebegin/matchdeck.git"

git add -A
t "git add"
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
t "git push+done"
echo "已發佈 $(date -u +%H:%MZ)"