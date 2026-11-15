#!/usr/bin/env node
/**
 * tagmerge.mjs — 把 memo 標籤補入即將用嚟 build 嘅 fixtures 檔
 *
 * 為咩要有呢個：標籤喺 fb/fixtures.mjs 已經 build-time 寫入，但 publish.sh 有兩條
 * 數據源（首選 fbapi → api-fixtures.json、後備本地 fixtures.json）。fbapi 嘅
 * /matches?full=1 只帶到部分場次嘅 memo（實測 600 行只有 182 行有 tags，本地係 344），
 * 結果頁面有 162 張卡明明有標籤卻空白（用戶 m18618 同一類問題）。
 * 所以不論用邊條來源，build 之前都用 data.db 將 tags／memoNote 補齊。
 *
 * 用法：node tagmerge.mjs fixtures.json|api-fixtures.json
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { DatabaseSync } from 'node:sqlite';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const target = process.argv[2] || 'fixtures.json';
const file = path.resolve(process.cwd(), target);
if (!fs.existsSync(file)) { console.log(`補標籤：找唔到 ${target}，跳過`); process.exit(0); }

const j = JSON.parse(fs.readFileSync(file, 'utf8'));
const rows = j.rows || j;

const tagMap = new Map();
try {
  const db = new DatabaseSync(path.join(HERE, '..', 'fb', 'data.db'), { readOnly: true });
  for (const r of db.prepare('SELECT mid, tags, note FROM memos').all()) {
    let tags = [];
    try { tags = JSON.parse(r.tags || '[]'); } catch { tags = []; }
    if (!Array.isArray(tags) || !tags.length) continue;
    tagMap.set(String(r.mid), { tags, note: r.note || '' });
  }
  db.close();
} catch (e) {
  console.log(`補標籤：讀 data.db 失敗（${e.message}），跳過`); process.exit(0);
}

let added = 0, kept = 0;
for (const r of rows) {
  const t = tagMap.get(String(r.id));
  if (!t) continue;
  if (Array.isArray(r.tags) && r.tags.length) { kept++; continue; }
  r.tags = t.tags;
  if (!r.memoNote) r.memoNote = t.note;
  added++;
}
fs.writeFileSync(file, JSON.stringify(j));
const have = rows.filter((r) => Array.isArray(r.tags) && r.tags.length).length;
console.log(`補標籤：${target} 原有 ${kept} → 補 ${added} → 共 ${have} 張卡有標籤（memo 表 ${tagMap.size} 筆）`);
