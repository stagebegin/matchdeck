#!/usr/bin/env bash
# deploy.sh — 把本地 repo 推到 GitHub，並嘗試開 Pages
# 用法：GH_TOKEN=xxx ./deploy.sh [remote-url]
# token 只經環境變數與記憶體，不寫入任何檔案、不進 git config。
set -euo pipefail

OWNER=stagebegin
REPO=matchdeck
TOKEN="${GH_TOKEN:?需要 GH_TOKEN 環境變數}"
REMOTE="${1:-https://github.com/$OWNER/$REPO.git}"
BRANCH=main

cd "$(dirname "$0")"

# 用一次性 credential helper 注入 token，避免寫進 .git/config
git -c "credential.helper=!f() { echo username=x-access-token; echo password=$TOKEN; }; f" \
  push "https://github.com/$OWNER/$REPO.git" "HEAD:$BRANCH" --force-with-lease 2>&1 | sed -E "s/${TOKEN//\//\\/}/***/g"

echo "--- pushed; 檢查 API ---"
curl -s -o /dev/null -w "repo: %{http_code}\n" \
  -H "Authorization: Bearer $TOKEN" "https://api.github.com/repos/$OWNER/$REPO"

echo "--- 嘗試開 Pages（已開會回 409）---"
curl -s -X POST -H "Authorization: Bearer $TOKEN" \
  -H "Accept: application/vnd.github+json" \
  "https://api.github.com/repos/$OWNER/$REPO/pages" \
  -d "{\"source\":{\"branch\":\"$BRANCH\",\"path\":\"/\"}}" \
  -w "\npages: %{http_code}\n" | tail -3