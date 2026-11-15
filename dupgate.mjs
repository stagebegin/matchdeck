#!/usr/bin/env node
/**
 * dupgate.mjs — 發佈閘：同一場比賽唔可以出兩張卡
 *
 * 用戶 m19439：「這場比賽重覆出現了」。馬會改期／重新掛牌會換 id，
 * 舊 id 仍然喺 db／API 源頭度，結果同一場出兩張卡。
 *
 * 判斷：同 comp ＋ 同主隊 ＋ 同客隊，開賽時間相差 ≤ 3 小時 → 重複。
 * 用法：node dupgate.mjs fixtures.json|api-fixtures.json
 */
import fs from 'node:fs';
import path from 'node:path';

const WIN_MS = 3 * 3600 * 1000;
const file = path.resolve(process.cwd(), process.argv[2] || 'fixtures.json');
if (!fs.existsSync(file)) { console.log(`重複卡閘：找唔到 ${file}，跳過`); process.exit(0); }

const j = JSON.parse(fs.readFileSync(file, 'utf8'));
const rows = j.rows || j;
const pk = (s) => {
  const t = String(s || '');
  return t.includes('T') ? Date.parse(t) : Date.parse(t.replace(' ', 'T') + '+08:00');
};

const groups = new Map();
for (const r of rows) {
  const k = `${r.comp}|${r.home}|${r.away}`;
  if (!groups.has(k)) groups.set(k, []);
  groups.get(k).push(r);
}

const bad = [];
for (const [k, list] of groups) {
  if (list.length < 2) continue;
  const t = list.map((r) => pk(r.kickoff || r.kick)).sort();
  for (let i = 1; i < t.length; i++) {
    if (Number.isFinite(t[i]) && Number.isFinite(t[i - 1]) && Math.abs(t[i] - t[i - 1]) <= WIN_MS) {
      bad.push({ key: k, ids: list.map((r) => r.id + '@' + (r.kickoff || r.kick)) });
      break;
    }
  }
}

if (!bad.length) { console.log(`重複卡閘：${rows.length} 場，0 場重複 ✓`); process.exit(0); }
console.log(`✗ 重複卡閘：${bad.length} 場同一比賽出多過一張卡（跑 node ../fb/dedupe.mjs --apply 清理）`);
for (const b of bad.slice(0, 8)) console.log(`  ${b.key}：${b.ids.join(' / ')}`);
process.exit(1);
