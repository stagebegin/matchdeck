// api-fetch.mjs — 由 fbapi 拉 /matches?full=1 落地成 fixtures.json 同構檔（方案 A 數據源）
// 用法：node api-fetch.mjs [--out api-fixtures.json]
// 失敗（API 唔喺）時 exit 1，由呼叫方決定 fallback 返本地 fixtures.json。
import fs from 'node:fs';
import path from 'node:path';

const HERE = import.meta.dirname;
const out = process.argv.includes('--out') ? process.argv[process.argv.indexOf('--out') + 1] : path.join(HERE, 'api-fixtures.json');

const prefix = fs.readFileSync(path.join(HERE, '..', 'api', 'api_prefix.txt'), 'utf8').trim();
const addr = process.env.FBAPI_ADDR || '127.0.0.1:8787';
const from = new Date(Date.now() - 4 * 86400000).toISOString().slice(0, 10);
const to = new Date(Date.now() + 7 * 86400000).toISOString().slice(0, 10);
const url = `http://${addr}/${prefix}/matches?from=${from}&to=${to}&full=1&limit=2000`;

const res = await fetch(url);
if (!res.ok) throw new Error(`fbapi HTTP ${res.status}`);
const d = await res.json();
if (!d.rows || !d.rows.length) throw new Error('fbapi 空 rows');
// kick：API 出 UTC（Z），fixtures.mjs 版係 +08:00——main.rs 的 kick_parts 直接切字串
// 顯示，必須轉返 +08:00 格先同舊版一像素不變（epoch_secs 認 +08:00 offset 亦正確）。
const toHK = (iso) => {
  const p = new Date(new Date(iso).getTime() + 8 * 3600 * 1000);
  return p.toISOString().replace('Z', '').replace(/(\.\d{3})?$/, '.000') + '+08:00';
};
for (const r of d.rows) r.kick = toHK(r.kick);

// has_fm（FotMob 有冇收錄此場）唔喺 db schema 內，由本地 fixtures.json 補回；
// 冇對應（例如 API 比本地窗更遠的場次）就當 true，即「未到時間、仍然會出預測」。
let local = {};
try {
  const lf = JSON.parse(fs.readFileSync(path.join(HERE, 'fixtures.json'), 'utf8'));
  for (const r of lf.rows || []) local[String(r.id)] = r;
} catch { /* 首次／冇本地檔 */ }
for (const r of d.rows) {
  const l = local[String(r.id)];
  r.has_fm = l ? !!l.has_fm : true;
  r.fm_lg = l ? (l.fm_lg ?? null) : null;
  if (l && l.note) r.note = l.note; // 未有預測的誠實原因也由本地 fixtures.json 提供
}
// fixtures.json 形狀：{generated, rows:[...]}；API rows 已同構（compName/minute/pred 全齊）
fs.writeFileSync(out, JSON.stringify({ generated: new Date().toISOString(), rows: d.rows }));
console.log(`api-fetch: ${d.rows.length} rows → ${out}`);
