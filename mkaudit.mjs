#!/usr/bin/env node
/** mkaudit.mjs — 把 fb/audit.json（監視器報告）渲染成 web/audit.html */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const OUT = path.join(HERE, 'audit.html');
let a;
try { a = JSON.parse(fs.readFileSync(path.join(HERE, '..', 'fb', 'audit.json'), 'utf8')); }
catch { console.log('冇 audit.json，略過'); process.exit(0); }

const esc = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
const when = new Date(a.at).toLocaleString('zh-HK', { timeZone: 'Asia/Hong_Kong' });
const c = a.counts || { fail: 0, warn: 0, info: 0 };

const byArea = new Map();
for (const i of a.issues || []) {
  if (!byArea.has(i.area)) byArea.set(i.area, []);
  byArea.get(i.area).push(i);
}
const order = { fail: 0, warn: 1, info: 2 };
const areas = [...byArea.entries()].sort((x, y) => order[x[1][0].sev] - order[y[1][0].sev]);

const block = ([area, list]) => {
  const sev = list[0].sev;
  const rows = list.map((i) => `<tr><td class="ref">${esc(i.ref)}</td><td>${esc(i.msg)}</td></tr>`).join('');
  return `<section class="area ${sev}">
  <h2>${sev === 'fail' ? '✗' : sev === 'warn' ? '⚠' : '·'} ${esc(area)} <span class="n">${list.length}</span></h2>
  <table><thead><tr><th>場次</th><th>說明</th></tr></thead><tbody>${rows}</tbody></table>
</section>`;
};

const html = `<!doctype html><html lang="zh-HK"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>監視報告 · MatchDeck</title>
<style>
:root{--bg:#0c1417;--card:#122025;--line:#1e3a41;--ink:#dcecef;--muted:#8aa6ac;--cyan:#4c9ba1;--warn:#f0b429;--bad:#ff7a5a}
*{box-sizing:border-box}
body{margin:0;background:radial-gradient(1200px 600px at 50% -10%,#163037,#0c1417 60%);color:var(--ink);font:14px/1.6 -apple-system,"PingFang HK","Noto Sans HK",sans-serif;padding:24px}
.wrap{max-width:1000px;margin:0 auto}
h1{font-size:20px;margin:0 0 4px}
.sub{color:var(--muted);font-size:12px;margin-bottom:18px}
.cards{display:flex;gap:10px;margin-bottom:20px;flex-wrap:wrap}
.kpi{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:10px 16px;min-width:110px}
.kpi b{display:block;font-size:22px;line-height:1.2}
.kpi span{color:var(--muted);font-size:11px}
.kpi.fail b{color:var(--bad)}.kpi.warn b{color:var(--warn)}.kpi.ok b{color:var(--cyan)}
.area{background:var(--card);border:1px solid var(--line);border-radius:14px;padding:14px 16px;margin-bottom:14px}
.area.fail{border-color:rgba(255,122,90,.45)}
.area.warn{border-color:rgba(240,180,41,.3)}
h2{font-size:14px;margin:0 0 10px;letter-spacing:.04em}
.area.fail h2{color:var(--bad)}.area.warn h2{color:var(--warn)}.area.info h2{color:var(--muted)}
.n{color:var(--muted);font-weight:400;font-size:12px}
table{width:100%;border-collapse:collapse;font-size:12.5px}
th{text-align:left;color:var(--muted);font-weight:500;font-size:11px;padding:4px 6px;border-bottom:1px solid var(--line)}
td{padding:5px 6px;border-bottom:1px solid rgba(30,58,65,.5);vertical-align:top}
tr:last-child td{border-bottom:0}
td.ref{white-space:nowrap;color:var(--cyan);width:42%}
footer{color:var(--muted);font-size:11px;text-align:center;margin-top:18px;line-height:1.8}
a{color:var(--cyan)}
</style></head><body><div class="wrap">
<h1>監視報告</h1>
<div class="sub">${esc(when)} 生成 · 檢查 ${a.checked ?? 0} 個未開賽／進行中預測（另有 ${a.finished ?? 0} 個已完場只作記錄）</div>
<div class="cards">
  <div class="kpi ${c.fail ? 'fail' : 'ok'}"><b>${c.fail}</b><span>需要處理</span></div>
  <div class="kpi ${c.warn ? 'warn' : 'ok'}"><b>${c.warn}</b><span>值得留意</span></div>
  <div class="kpi"><b>${c.info}</b><span>資訊</span></div>
  <div class="kpi"><b>${a.preds}</b><span>預測總數</span></div>
</div>
${areas.map(block).join('') || '<p style="color:var(--muted)">冇發現唔合理嘅預測。</p>'}
<footer>
自動每日／每次發佈前檢查（<code>fb/sanity.mjs</code>）。<br>
規則：樣本 &lt;300 · 模型總入球離 2.0–3.4 · λ 極端 · 機率同 Elo 方向矛盾 · 罕見高機率 · 和局率離 14–32% · 比分過度集中 · 校準層改動 &gt;12pp · 亞盤 |E|&gt;6% · 跨級別盃賽未併池 · 重複預測 · 同隊同日兩場 · 快照停滯 · 預測停產 · 2 小時內開賽仍冇預測。<br>
<a href="index.html">← 返回賽程</a>
</footer>
</div></body></html>`;

fs.writeFileSync(OUT, html);
console.log(`wrote ${path.relative(process.cwd(), OUT)} (${html.length} bytes)`);
