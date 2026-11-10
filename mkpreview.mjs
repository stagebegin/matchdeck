#!/usr/bin/env node
/**
 * mkpreview.mjs — 重生 web/preview-fixtures.json（離線預覽用嘅樣本集）
 *
 * 用途：畀人（或者我自己）喺唔接 API 嘅情況下，用一份細樣本睇卡面／亞盤表／校準行嘅效果。
 * 內容 = fixtures.json 內最近若干場（每個狀態各取樣）＋ 現行校準器（cal/*.json）摘要。
 *
 *   node mkpreview.mjs                # 每個狀態各 2 場
 *   node mkpreview.mjs --per 3        # 每個狀態各 3 場
 *   node mkpreview.mjs --all          # 全部場次
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const FB = path.join(HERE, '..', 'fb');
const argv = process.argv.slice(2);
const opt = (k, d) => { const i = argv.indexOf('--' + k); return i < 0 ? d : (argv[i + 1] ?? true); };
const has = (k) => argv.includes('--' + k);

const fx = JSON.parse(fs.readFileSync(path.join(HERE, 'fixtures.json'), 'utf8'));
const rows = fx.rows || [];

let picked;
if (has('all')) {
  picked = rows;
} else {
  const per = Number(opt('per', 2));
  const buckets = new Map();
  for (const r of rows) {
    const k = r.status || 'unknown';
    if (!buckets.has(k)) buckets.set(k, []);
    buckets.get(k).push(r);
  }
  picked = [];
  for (const list of buckets.values()) picked = picked.concat(list.slice(0, per));
  picked.sort((a, b) => new Date(a.kick) - new Date(b.kick));
}

// 校準器摘要（cal/*.json + pooled.json）
const calDir = path.join(FB, 'cal');
const calib = [];
let pooled = null;
if (fs.existsSync(calDir)) {
  for (const f of fs.readdirSync(calDir).sort()) {
    if (f === 'pooled.json') { pooled = JSON.parse(fs.readFileSync(path.join(calDir, f), 'utf8')); continue; }
    if (!f.endsWith('.json')) continue;
    const j = JSON.parse(fs.readFileSync(path.join(calDir, f), 'utf8'));
    if (!j.metrics) continue;
    calib.push({ lg: j.lg, name: j.name, n: j.n_samples, ...j.metrics, n_rows: j.n_rows });
  }
}

const counts = {};
for (const r of picked) counts[r.status] = (counts[r.status] || 0) + 1;

const out = {
  generated: new Date().toISOString(),
  counts,
  calib,
  pooled,
  rows: picked,
};

const dest = path.join(HERE, 'preview-fixtures.json');
fs.writeFileSync(dest, JSON.stringify(out, null, 1));
console.log(`已寫入 preview-fixtures.json：${picked.length} 場（${JSON.stringify(counts)}）、校準器 ${calib.length} 個、pooled ${pooled ? '有' : '無'}`);
