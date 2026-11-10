#!/usr/bin/env node
/**
 * mkeuropa.mjs — 出歐聯／歐霸「聯賽階段＋淘汰賽」模擬結果頁（Step 3 新賽制）
 *
 *   node mkeuropa.mjs            # 讀 fb/tournament/eu-*.json → web/europa.html
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const TDIR = path.join(HERE, '..', 'fb', 'tournament');

const esc = (s) => String(s == null ? '' : s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const p1 = (x) => (x * 100).toFixed(1) + '%';
const p0 = (x) => (x * 100).toFixed(0) + '%';
const pad = (n) => String(n).padStart(2, '0');
const hk = (iso) => {
  const d = new Date(iso);
  if (isNaN(d)) return esc(iso);
  const t = new Date(d.getTime() + 8 * 3600_000);
  return `${pad(t.getUTCMonth() + 1)}-${pad(t.getUTCDate())} ${pad(t.getUTCHours())}:${pad(t.getUTCMinutes())}`;
};

const files = fs.readdirSync(TDIR).filter((f) => /^eu-.*\.json$/.test(f)).sort();
if (!files.length) { console.log('冇 eu-*.json，略過'); process.exit(0); }

function section(T) {
  const byChamp = [...T.teams].sort((a, b) => b.champ - a.champ);
  const maxChamp = Math.max(...T.teams.map((t) => t.champ), 0.01);

  const champBars = byChamp.slice(0, 10).map((t) => `<div class="cb">
    <span class="cn">${esc(t.name)}</span>
    <span class="cbar"><i style="width:${(t.champ / maxChamp) * 100}%"></i></span>
    <span class="cv">${p1(t.champ)}</span>
  </div>`).join('');

  const rows = T.teams.map((t, i) => `<tr class="${i < 8 ? 'seed' : (i < 24 ? 'po' : '')}">
    <td class="num dim">${i + 1}</td>
    <td class="tm">${esc(t.name)}${t.elo ? `<span class="elo">${t.elo}</span>` : ''}${t.hasHist ? '' : '<span class="nod">無數據</span>'}</td>
    <td class="num">${p0(t.p8)}</td>
    <td class="num">${p0(t.po)}</td>
    <td class="num">${p0(t.pf)}</td>
    <td class="num">${p0(t.sf)}</td>
    <td class="num">${p1(t.final)}</td>
    <td class="num ch">${p1(t.champ)}</td>
  </tr>`).join('');

  const fixtures = T.fixtures.map((f) => `<tr>
    <td class="mono">${hk(f.utc)}</td>
    <td class="mono">R${esc(f.round)}</td>
    <td>${esc(f.home)} <span class="vs">v</span> ${esc(f.away)}</td>
    <td class="scores">${f.lowConf ? '<span class="nodata">有球隊冇歷史數據 · 唔出比分</span>' : f.top.map((x) => `<span class="sc">${esc(x.score)} <b>${p0(x.p)}</b></span>`).join('')}</td>
  </tr>`).join('');

  const legend = (T.legend || []).map((l) => `<li>${esc(l.title)}：第 ${l.indices.map((i) => i + 1).join('、')} 名</li>`).join('');
  const assumptions = (T.assumptions || []).map((a) => `<li>${esc(a)}</li>`).join('');

  return `<div class="cup">
<div class="hero">
  <h1>${esc(T.name)} ${esc(T.season)} · 聯賽階段模擬</h1>
  <div class="sub">Monte Carlo ${T.sims.toLocaleString()} 次（seed ${T.seed}）· 強度池 ${T.model.poolMatches.toLocaleString()} 場 / ${T.model.teams} 隊 · 主場係數 ×${T.model.H} · 每隊基準 ${T.model.base} 球</div>
  <div class="gen">產生時間 ${esc(T.generatedAt)} · 已完場用真實賽果，未完場由模型抽樣</div>
</div>

<div class="method">
  <div class="mcard"><b>① 聯賽階段</b><span>36 隊 8 輪（每隊 8 場）。積分 → 淨勝球 → 入球 → 隨機排位。</span></div>
  <div class="mcard"><b>② 席位</b><span>第 1-8 名直接入 1/8 回合；第 9-24 名打附加賽（9 對 24、10 對 23 …… 16 對 17），兩回合。</span></div>
  <div class="mcard"><b>③ 淘汰賽</b><span>1/8 回合由 8 支種子對附加賽勝者抽籤決定（FotMob 未提供已抽對賽）；1/4 兩回合；1/2 同決賽單回合中立場。</span></div>
  <div class="mcard"><b>④ 限制</b><span>和局以十二碼 50/50 決勝（唔模擬加時）；抽籤同主場為隨機假設；模型難以穩定贏市場。</span></div>
</div>
<div class="warn">本頁是統計模擬，<b>不構成任何投注建議</b>。市場賠率從未參與運算。</div>

<div class="sect-title">奪冠機率（前十）</div>
<div class="cbwrap">${champBars}</div>

<div class="sect-title">聯賽階段排名 · 席位 · 晉級</div>
<div class="fixt"><table class="stand">
<thead><tr><th class="num">#</th><th>球隊</th><th class="num">前 8</th><th class="num">附加賽</th><th class="num">1/4</th><th class="num">1/2</th><th class="num">決賽</th><th class="num">奪冠</th></tr></thead>
<tbody>${rows}</tbody></table></div>

<div class="sect-title">未踢場次 · 最可能比分</div>
<div class="fixt"><table><thead><tr><th>開賽（北京時間）</th><th>輪次</th><th>對賽</th><th>典型比分</th></tr></thead><tbody>${fixtures}</tbody></table></div>

<div class="sect-title">賽制席位（FotMob legend）</div>
<ol class="assump">${legend}</ol>
<div class="sect-title">模型假設（全部可查）</div>
<ol class="assump">${assumptions}</ol>
</div>`;
}

const HTML = `<!DOCTYPE html>
<html lang="zh-Hant">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>歐聯／歐霸 聯賽階段模擬</title>
<style>
:root{--bg:#0e1417;--card:#151d21;--card2:#1b252a;--line:#26333a;--txt:#e6edf0;--dim:#93a7b0;--accent:#4c9ba1;--green:#4fbf87;--yellow:#e5b454}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--txt);font:14px/1.6 -apple-system,BlinkMacSystemFont,"Segoe UI","Noto Sans TC","PingFang TC","Microsoft JhengHei",sans-serif}
.wrap{max-width:1180px;margin:0 auto;padding:24px 18px 60px}
.cup{margin-bottom:46px}
.hero{padding:32px 26px 24px;border-radius:16px;margin-bottom:16px;background:linear-gradient(150deg,#16323a 0%,#12242a 55%,#101a1f 100%);border:1px solid var(--line);position:relative;overflow:hidden}
.hero h1{margin:0 0 8px;font-size:25px;letter-spacing:.04em;font-weight:650}
.hero .sub{color:var(--dim);font-size:13.5px}
.hero .gen{color:#6f858d;font-size:12px;margin-top:10px}
.method{display:grid;grid-template-columns:repeat(auto-fit,minmax(225px,1fr));gap:12px;margin-bottom:12px}
.mcard{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:14px 16px}
.mcard b{display:block;font-size:13.5px;margin-bottom:6px;color:#cfe3e8}
.mcard span{color:var(--dim);font-size:12.5px;line-height:1.65}
.warn{margin:0 0 18px;padding:12px 16px;border-radius:10px;font-size:12.8px;background:rgba(229,180,84,.1);border:1px solid rgba(229,180,84,.35);color:#f0cf8d}
.sect-title{font-size:15px;font-weight:600;margin:26px 0 12px;padding-left:10px;border-left:3px solid var(--accent)}
.cbwrap{display:grid;gap:6px;background:var(--card);border:1px solid var(--line);border-radius:12px;padding:14px 16px}
.cb{display:grid;grid-template-columns:150px 1fr 62px;gap:10px;align-items:center;font-size:13px}
.cn{color:#d7e6ea}
.cbar{height:9px;background:#1d282e;border-radius:5px;overflow:hidden}
.cbar i{display:block;height:100%;background:linear-gradient(90deg,#4c9ba1,#7fd3c6)}
.cv{text-align:right;color:#bfe6ea;font-variant-numeric:tabular-nums}
.fixt{background:var(--card);border:1px solid var(--line);border-radius:12px;overflow-x:auto}
table{border-collapse:collapse;width:100%;font-size:13px}
th,td{padding:9px 10px;text-align:left;border-bottom:1px solid var(--line);white-space:nowrap}
th{color:var(--dim);font-weight:500;font-size:12.5px}
tbody tr:last-child td{border-bottom:0}
.stand tr.seed td{background:rgba(79,191,135,.07)}
.stand tr.po td{background:rgba(76,155,161,.06)}
.num{text-align:right;font-variant-numeric:tabular-nums}
.dim{color:#6f858d}
.tm{font-weight:550}
.elo{color:#6f858d;font-size:11.5px;margin-left:6px;font-variant-numeric:tabular-nums}
.ch{color:#7fd3c6;font-weight:600}
.nod{color:#e5b454;font-size:11px;margin-left:6px}
.nodata{color:#e5b454;font-size:12px}
.mono{font-variant-numeric:tabular-n;color:var(--dim)}
.vs{color:#6f858d;padding:0 2px}
.sc{display:inline-block;margin-right:8px;padding:2px 8px;border-radius:6px;background:#1d282e;font-variant-numeric:tabular-nums;font-size:12.5px}
.sc b{color:#7fd3c6;font-weight:600}
.assump{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:14px 16px 14px 34px;color:var(--dim);font-size:12.8px}
.assump li{margin:5px 0}
footer{margin-top:30px;color:#6f858d;font-size:12px;text-align:center;line-height:1.8}
</style>
</head>
<body><div class="wrap">
${files.map((f) => section(JSON.parse(fs.readFileSync(path.join(TDIR, f), 'utf8')))).join('\n')}
<footer>Step 3 歐聯／歐霸聯賽階段模擬 · 模型輸出非投注建議 · 數據源 FotMob</footer>
</div></body></html>`;

const OUT = path.join(HERE, 'europa.html');
fs.writeFileSync(OUT, HTML);
console.log(`已寫入 europa.html（${HTML.length.toLocaleString()} bytes，${files.length} 個賽事）`);
