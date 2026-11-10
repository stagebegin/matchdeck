#!/usr/bin/env node
/**
 * mkcups.mjs — 把 fb/tournament/cup-<comp>.json（Step 3 盃賽模擬結果）出成一頁靜態盃賽總覽
 *
 *   node mkcups.mjs                 # 全部盃賽 → web/cups.html
 *   node mkcups.mjs --out cups.html
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const argv = process.argv.slice(2);
const opt = (k, d) => { const i = argv.indexOf('--' + k); return i < 0 ? d : (argv[i + 1] ?? true); };

const TDIR = path.join(HERE, '..', 'fb', 'tournament');
const OUT = path.join(HERE, opt('out', 'cups.html'));
const files = fs.readdirSync(TDIR).filter((f) => /^cup-.*\.json$/.test(f)).sort();
const CUPS = files.map((f) => JSON.parse(fs.readFileSync(path.join(TDIR, f), 'utf8')));
if (!CUPS.length) { console.error('冇盃賽結果（先跑 fb/cups.mjs --all）'); process.exit(1); }

const esc = (s) => String(s == null ? '' : s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const p1 = (x) => ((x || 0) * 100).toFixed(1) + '%';
const p0 = (x) => ((x || 0) * 100).toFixed(0) + '%';
const pad = (n) => String(n).padStart(2, '0');
const hk = (iso) => {
  const d = new Date(iso);
  if (isNaN(d)) return esc(iso);
  const t = new Date(d.getTime() + 8 * 3600_000);
  return `${pad(t.getUTCMonth() + 1)}-${pad(t.getUTCDate())} ${pad(t.getUTCHours())}:${pad(t.getUTCMinutes())}`;
};
const anchor = (s) => 'cup-' + String(s).replace(/[^A-Za-z0-9]/g, '');

const FORMAT_LABEL = { ko: '純淘汰賽', 'group+ko': '分組＋淘汰', league: '聯賽階段' };
const STAGE_LABEL = (c) => {
  if (c.format === 'league') return '聯賽階段進行中（未有淘汰賽抽籤）';
  if (c.format === 'group+ko' && !c.rounds || (c.format === 'group+ko' && !c.rounds.length)) return '分組賽進行中（淘汰賽未抽籤）';
  return `當前：${c.stage}`;
};

function teamTable(c) {
  const head = c.format === 'league'
    ? '<th>球隊</th><th class="num">首名</th><th class="num">前 4</th><th class="num">前 8</th><th class="num">期望積分</th>'
    : c.format === 'group+ko'
      ? '<th>球隊</th><th>組</th><th class="num">分組首名</th><th class="num">出線</th><th class="num">奪冠</th>'
      : '<th>球隊</th><th class="num">晉級下一圈</th><th class="num">入決賽</th><th class="num">奪冠</th>';
  const rows = c.teams.slice(0, c.format === 'ko' ? 20 : 40).map((t, i) => {
    const nm = `<td class="tm"><span class="rk">${i + 1}</span>${esc(t.name)}${t.hasHist === false ? '<span class="nod" title="呢隊冇歷史數據，用聯賽平均值先驗">無數據</span>' : ''}${t.elo ? `<span class="elo">${Math.round(t.elo)}</span>` : ''}</td>`;
    if (c.format === 'league') {
      return `<tr>${nm}<td class="num">${p1(t.p1)}</td><td class="num">${p1(t.p4)}</td><td class="num">${p1(t.p8)}</td><td class="num">${(t.expPts || 0).toFixed(1)}</td></tr>`;
    }
    if (c.format === 'group+ko') {
      return `<tr>${nm}<td class="mono">${esc(t.group)}</td><td class="num">${p1(t.p1)}</td><td class="num">${p1((t.p1 || 0) + (t.p2 || 0))}</td><td class="num ch">${p1(t.champ)}</td></tr>`;
    }
    return `<tr>${nm}<td class="num">${p1(t.pNext)}</td><td class="num">${p1(t.pFinal)}</td><td class="num ch">${p1(t.champ)}</td></tr>`;
  }).join('');
  return `<table class="gt"><thead><tr>${head}</tr></thead><tbody>${rows}</tbody></table>`;
}

function cupCard(c) {
  const fixtures = (c.fixtures || []).slice(0, 40).map((f) => `<tr>
    <td class="mono">${hk(f.utc)}</td>
    <td class="mono">${esc(f.round)}${f.legs > 1 ? ' · 兩回合' : ''}</td>
    <td>${esc(f.home)} <span class="vs">v</span> ${esc(f.away)}</td>
    <td class="scores">${f.lowConf ? '<span class="nodata">有球隊冇歷史數據 · 唔出比分</span>' : (f.top || []).map((x) => `<span class="sc">${esc(x.score)} <b>${p0(x.p)}</b></span>`).join('')}</td>
  </tr>`).join('');
  const noHist = (c.teams || []).filter((t) => t.hasHist === false).length;
  const lowFx = (c.fixtures || []).filter((f) => f.lowConf).length;
  const missing = (c.missing && c.missing.length)
    ? `<div class="warn">${noHist}／${(c.teams || []).length} 隊冇歷史數據（用聯賽平均值先驗，唔係數據）：${esc(c.missing.join('、'))}${lowFx ? `；本輪 ${lowFx}／${(c.fixtures || []).length} 場因此<b>唔出典型比分</b>` : ''}</div>`
    : '';
  return `<section class="cup" id="${anchor(c.comp)}">
  <div class="chead">
    <div>
      <h2>${esc(c.name)} <span class="fmt">${esc(FORMAT_LABEL[c.format] || c.format)}</span></h2>
      <div class="chsub">馬會代號 ${esc(c.comp)} · ${esc(c.season)} · ${esc(STAGE_LABEL(c))} · 未踢 ${(c.fixtures || []).length} 場</div>
    </div>
    <div class="chstat"><b>${(c.sims || 0).toLocaleString()}</b><span>次模擬</span></div>
  </div>
  <div class="cbody">
    ${missing}
    ${teamTable(c)}
    ${fixtures ? `<div class="sect-title sm">未踢場次 · 最可能比分（前 3）</div><div class="fixt"><table><thead><tr><th>開賽（北京時間）</th><th>輪次</th><th>對賽</th><th>典型比分</th></tr></thead><tbody>${fixtures}</tbody></table></div>` : ''}
    <details class="assump"><summary>模型假設（${(c.assumptions || []).length} 條，全部可查）</summary><ol>${(c.assumptions || []).map((a) => `<li>${esc(a)}</li>`).join('')}</ol></details>
  </div>
</section>`;
}

const nav = CUPS.map((c) => `<a href="#${anchor(c.comp)}">${esc(c.name)}</a>`).join('');
const totalSims = CUPS.reduce((s, c) => s + (c.sims || 0), 0);
const model = CUPS[0] ? CUPS[0].model : { n: 0, teams: 0, H: 1, base: 0 };

const HTML = `<!DOCTYPE html>
<html lang="zh-Hant">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>盃賽模擬總覽 · MatchDeck</title>
<style>
:root{--bg:#0e1417;--card:#151d21;--card2:#1b252a;--line:#26333a;--txt:#e6edf0;--dim:#93a7b0;--accent:#4c9ba1;--green:#4fbf87;--yellow:#e5b454}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--txt);font:14px/1.6 -apple-system,BlinkMacSystemFont,"Segoe UI","Noto Sans TC","PingFang TC","Microsoft JhengHei",sans-serif}
.wrap{max-width:1180px;margin:0 auto;padding:24px 18px 60px}
.hero{padding:32px 26px 24px;border-radius:16px;margin-bottom:16px;background:linear-gradient(150deg,#16323a 0%,#12242a 55%,#101a1f 100%);border:1px solid var(--line);position:relative;overflow:hidden}
.hero:after{content:"";position:absolute;right:-70px;top:-70px;width:260px;height:260px;border-radius:50%;background:radial-gradient(circle,rgba(76,155,161,.22),transparent 68%)}
.hero h1{margin:0 0 8px;font-size:26px;letter-spacing:.04em;font-weight:650}
.hero .sub{color:var(--dim);font-size:13.5px}
.hero .gen{color:#6f858d;font-size:12px;margin-top:10px}
.nav{display:flex;flex-wrap:wrap;gap:8px;margin:14px 0 18px}
.nav a{color:#bfe6ea;text-decoration:none;font-size:12.5px;padding:6px 12px;border-radius:999px;background:var(--card);border:1px solid var(--line)}
.nav a:hover{border-color:var(--accent);color:#fff}
.method{display:grid;grid-template-columns:repeat(auto-fit,minmax(225px,1fr));gap:12px;margin-bottom:12px}
.mcard{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:14px 16px}
.mcard b{display:block;font-size:13.5px;margin-bottom:6px;color:#cfe3e8}
.mcard span{color:var(--dim);font-size:12.5px;line-height:1.65}
.warn{margin:0 0 12px;padding:10px 14px;border-radius:10px;font-size:12.5px;background:rgba(229,180,84,.1);border:1px solid rgba(229,180,84,.35);color:#f0cf8d}
.cup{background:var(--card);border:1px solid var(--line);border-radius:14px;margin:18px 0;overflow:hidden;scroll-margin-top:12px}
.chead{display:flex;justify-content:space-between;align-items:center;gap:16px;padding:14px 18px;background:var(--card2);border-bottom:1px solid var(--line)}
.chead h2{margin:0;font-size:16.5px;font-weight:600;letter-spacing:.02em}
.fmt{font-size:11px;font-weight:500;color:#9fd8d0;background:rgba(76,155,161,.16);border:1px solid rgba(76,155,161,.4);border-radius:999px;padding:2px 9px;margin-left:8px;vertical-align:middle}
.chsub{color:var(--dim);font-size:12.3px;margin-top:5px}
.chstat{text-align:right;color:var(--dim);font-size:11.5px;line-height:1.3}
.chstat b{display:block;font-size:17px;color:#bfe6ea;font-variant-numeric:tabular-nums}
.cbody{padding:14px 18px 18px}
.sect-title{font-size:14px;font-weight:600;margin:18px 0 10px;padding-left:10px;border-left:3px solid var(--accent)}
.sect-title.sm{font-size:13px;margin:16px 0 8px}
table{border-collapse:collapse;width:100%;font-size:13px}
th,td{padding:8px 10px;text-align:left;border-bottom:1px solid var(--line);white-space:nowrap}
th{color:var(--dim);font-weight:500;font-size:12.3px}
.gt tbody tr:last-child td{border-bottom:0}
.tm{font-weight:550}
.rk{display:inline-block;min-width:18px;color:#5d737b;font-size:11px;font-variant-numeric:tabular-nums}
.elo{color:#6f858d;font-size:11px;margin-left:6px;font-variant-numeric:tabular-nums}
.nod{font-size:10.5px;color:#f0cf8d;background:rgba(229,180,84,.12);border:1px solid rgba(229,180,84,.35);border-radius:5px;padding:1px 5px;margin-left:6px}
.nodata{color:#f0cf8d;font-size:12px}
.num{text-align:right;font-variant-numeric:tabular-nums}
.ch{color:#7fd3c6;font-weight:600}
.fixt{background:#121a1e;border:1px solid var(--line);border-radius:10px;overflow-x:auto}
.mono{font-variant-numeric:tabular-nums;color:var(--dim)}
.vs{color:#6f858d;padding:0 2px}
.sc{display:inline-block;margin-right:8px;padding:2px 8px;border-radius:6px;background:#1d282e;font-variant-numeric:tabular-nums;font-size:12.5px}
.sc b{color:#7fd3c6;font-weight:600}
.assump{margin-top:16px;background:#121a1e;border:1px solid var(--line);border-radius:10px;padding:10px 16px;color:var(--dim);font-size:12.5px}
.assump summary{cursor:pointer;color:#bfe6ea}
.assump ol{padding-left:18px}
.assump li{margin:5px 0}
footer{margin-top:30px;color:#6f858d;font-size:12px;text-align:center;line-height:1.8}
</style>
</head>
<body><div class="wrap">
<div class="hero">
  <h1>盃賽模擬總覽</h1>
  <div class="sub">${CUPS.length} 個賽事 · 合共 ${totalSims.toLocaleString()} 次 Monte Carlo · 強度池 ${model.n} 場 / ${model.teams} 隊（跨聯賽近兩季）· 主場係數 ×${model.H}</div>
  <div class="gen">已完場用真實賽果；未踢場次由 Dixon-Coles 泊松（ρ=−0.08）抽樣。產生 ${esc(CUPS[0] ? CUPS[0].generatedAt : '')}</div>
</div>
<div class="nav">${nav}</div>
<div class="method">
  <div class="mcard"><b>① 隊力合成</b><span>Elo 0.70 ＋ 攻防擬合 0.20 ＋ 近況 0.10（近 365 日場均積分）。年齡／教練戰術／傷停冇涵蓋全部球隊的可靠數據源，設 0 並在此申報。</span></div>
  <div class="mcard"><b>② 逐場模擬</b><span>Step 1 的 Dixon-Coles 泊松抽樣入球；兩回合按合計入球，和則十二碼 50/50；決賽與四強中立場。</span></div>
  <div class="mcard"><b>③ 賽制</b><span>按 FotMob 抓到的真實輪次／分組結構；下一圈未抽籤就隨機抽籤＋主場隨機（馬會同 FotMob 都未提供未來對賽）。</span></div>
  <div class="mcard"><b>④ 限制</b><span>低組別球隊樣本少（甚至只有 1 場），機率可信度較低；模型難以穩定贏市場，唔會用市場賠率校正。</span></div>
</div>
<div class="warn">本頁是統計模擬，<b>不構成任何投注建議</b>。機率只反映模型假設下的相對強弱，唔代表真實結果；市場賠率從未參與運算。</div>
${CUPS.map(cupCard).join('\n')}
<footer>Step 3 盃賽模擬 · 模型輸出非投注建議 · 賽制數據源 FotMob＋馬會賽程</footer>
</div></body></html>`;

fs.writeFileSync(OUT, HTML);
console.log(`→ ${OUT}（${CUPS.length} 個賽事，${(HTML.length / 1024).toFixed(0)} KB）`);
