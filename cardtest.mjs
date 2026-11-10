#!/usr/bin/env node
/**
 * cardtest.mjs — 頁面層守則閘（每次 build 後跑）
 *
 * 檢查：
 *  ① 每張卡（正常卡同 ghost 卡）都必須有 data-id —— 否則 60 秒自動更新永遠唔會理佢
 *     （曾發生：ghost 卡冇 data-id，「無歷史數據」嘅場永遠卡喺「未開賽」）
 *  ② data.json 嘅 id 必須全部對得上一張卡（已完場但冇預測嘅場係刻意唔出卡，容許）
 *  ③ 每張卡嘅 comp／data-status 必須存在（過濾同自動更新靠佢）
 *
 *   node cardtest.mjs            用 dist/index.html + dist/data.json
 *   node cardtest.mjs --out dist/index.html
 */
import fs from 'node:fs';
import path from 'node:path';

const HERE = import.meta.dirname;
const argv = process.argv.slice(2);
const opt = (k, d) => { const i = argv.indexOf('--' + k); return i < 0 ? d : (argv[i + 1] ?? true); };
const htmlPath = opt('out', path.join(HERE, 'dist', 'index.html'));
const dataPath = path.join(path.dirname(htmlPath), 'data.json');

const html = fs.readFileSync(htmlPath, 'utf8');
const rows = JSON.parse(fs.readFileSync(dataPath, 'utf8'));
const list = Array.isArray(rows) ? rows : (rows.rows || Object.values(rows));

const problems = [];

// ① 每張卡都要有 data-id
const cards = [...html.matchAll(/<article class="card([^"]*)"([^>]*)>/g)].map((m) => ({ cls: m[1], attrs: m[2] }));
for (const c of cards) {
  if (!/data-id="/.test(c.attrs)) problems.push('有卡冇 data-id（自動更新會漏咗佢）：' + c.cls.trim());
  if (!/data-status="/.test(c.attrs)) problems.push('有卡冇 data-status（分頁搬卡會壞）：' + c.cls.trim());
  if (!/data-comp="/.test(c.attrs)) problems.push('有卡冇 data-comp（過濾會壞）：' + c.cls.trim());
}

// ② data.json 嘅場次：每個 id 要喺 html 有卡，或者係「已完場但冇預測」（刻意收埋）
const inHtml = new Set([...html.matchAll(/data-id="(\d+)"/g)].map((m) => m[1]));
let hidden = 0;
for (const r of list) {
  const id = String(r.id);
  if (inHtml.has(id)) continue;
  const fin = r.fin || r.base;
  const done = r.status === 'ft' || r.status === 'post';
  if (done && !fin) { hidden++; continue; } // 刻意收埋（冇得補跑）
  problems.push(`data.json 有場但頁面冇卡：${id}（${r.status}）`);
}

// ③ 卡數統計
const nCards = cards.length;
console.log(`卡片：${nCards} 張（全部有 data-id／data-status／data-comp）`);
console.log(`data.json：${list.length} 場 → 頁面有卡 ${list.length - hidden} 場，已完場冇預測而收埋 ${hidden} 場`);

for (const p of problems.slice(0, 20)) console.log('✗ ' + p);
if (problems.length) {
  console.log(`\n✗ 頁面層有 ${problems.length} 個問題`);
  process.exit(1);
}
console.log('\n✓ 頁面層檢查全部通過');
