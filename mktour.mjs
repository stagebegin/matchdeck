#!/usr/bin/env node
/**
 * mktour.mjs — 把 fb/tournament/<league>-<season>.json（Step 3 模擬結果）出成靜態頁
 *
 *   node mktour.mjs                          # 最新一份 → dist/tournament.html
 *   node mktour.mjs --in ../fb/tournament/9806-2026-2027.json --out dist/tournament.html
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const argv = process.argv.slice(2);
const opt = (k, d) => { const i = argv.indexOf('--' + k); return i < 0 ? d : (argv[i + 1] ?? true); };

const TDIR = path.join(HERE, '..', 'fb', 'tournament');
// 只揀國際／單一賽事結果（cup-*.json 係盃賽，由 mkcups.mjs 處理）
const tourFiles = fs.readdirSync(TDIR).filter((f) => /^\d.*\.json$/.test(f)).sort();
const IN = opt('in', null) ? path.resolve(opt('in')) : path.join(TDIR, tourFiles.pop());
const OUT = path.join(HERE, opt('out', 'tournament.html')); // 放 repo 根目錄（GitHub Pages 直接服務，dist/ 係 gitignore）
const T = JSON.parse(fs.readFileSync(IN, 'utf8'));

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

const groups = [...new Set(T.teams.map((t) => t.group))].sort((a, b) => a - b);
const byChamp = [...T.teams].sort((a, b) => b.champ - a.champ);
const maxChamp = Math.max(...T.teams.map((t) => t.champ), 0.01);

function teamRow(t) {
  const bars = [['首名', t.p1], ['次名', t.p2], ['第三', t.p3], ['第四', t.p4]]
    .map(([k, v]) => `<div class="px"><span class="pk">${k}</span><span class="pv">${p0(v)}</span></div>`).join('');
  return `<tr>
    <td class="tm">${esc(t.name)}${t.rating ? `<span class="elo">${t.rating.elo}</span>` : ''}</td>
    <td><div class="mini">${bars}</div></td>
    <td class="num qf">${p1(t.qf)}</td>
    <td class="num">${p1(t.sf)}</td>
    <td class="num">${p1(t.final)}</td>
    <td class="num ch">${p1(t.champ)}</td>
  </tr>`;
}

const groupHtml = groups.map((g) => {
  const rows = T.teams.filter((t) => t.group === g).sort((a, b) => b.p1 - a.p1);
  return `<div class="gcard">
    <div class="ghead">第 ${g} 組</div>
    <table class="gt">
      <thead><tr><th>球隊</th><th>分組名次機率</th><th class="num">出線</th><th class="num">四強</th><th class="num">決賽</th><th class="num">奪冠</th></tr></thead>
      <tbody>${rows.map(teamRow).join('')}</tbody>
    </table>
  </div>`;
}).join('');

const champBars = byChamp.slice(0, 10).map((t) => `<div class="cb">
  <span class="cn">${esc(t.name)}</span>
  <span class="cbar"><i style="width:${(t.champ / maxChamp) * 100}%"></i></span>
  <span class="cv">${p1(t.champ)}</span>
</div>`).join('');

const fixtureRows = T.fixtures.map((f) => `<tr>
  <td class="mono">${hk(f.utc)}</td>
  <td class="mono">G${esc(f.group)}</td>
  <td>${esc(f.home)} <span class="vs">v</span> ${esc(f.away)}</td>
  <td class="scores">${f.top.map((x) => `<span class="sc">${esc(x.score)} <b>${p0(x.p)}</b></span>`).join('')}</td>
</tr>`).join('');

const finalScores = T.finalScores && T.finalScores.length
  ? `<div class="sect-title">決賽最常見比分（中立場）</div><div class="fsbox">${T.finalScores.map((x) => `<span class="sc big">${esc(x.score)} <b>${p0(x.p)}</b></span>`).join('')}</div>`
  : '';

const sensHtml = T.sensitivity ? `<div class="sect-title">敏感度：主場優勢</div>
  <div class="sensbox">${Object.entries(T.sensitivity).map(([k, v]) => `<div class="scard"><b>${esc(k)}</b><div>${v.map((x) => `${esc(x.name)} <em>${x.champ}%</em>`).join('、')}</div></div>`).join('')}</div>` : '';

const assumptions = (T.assumptions || []).map((a) => `<li>${esc(a)}</li>`).join('');
const missing = (T.missing && T.missing.length) ? `<div class="warn">以下球隊冇歷史數據，已用先驗（強度 0）：${esc(T.missing.join('、'))}</div>` : '';

const HTML = `<!DOCTYPE html>
<html lang="zh-Hant">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>${esc(T.name)} ${esc(T.season)} · 盃賽模擬</title>
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
.method{display:grid;grid-template-columns:repeat(auto-fit,minmax(225px,1fr));gap:12px;margin-bottom:12px}
.mcard{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:14px 16px}
.mcard b{display:block;font-size:13.5px;margin-bottom:6px;color:#cfe3e8}
.mcard span{color:var(--dim);font-size:12.5px;line-height:1.65}
.warn{margin:0 0 18px;padding:12px 16px;border-radius:10px;font-size:12.8px;background:rgba(229,180,84,.1);border:1px solid rgba(229,180,84,.35);color:#f0cf8d}
.sect-title{font-size:15px;font-weight:600;margin:26px 0 12px;padding-left:10px;border-left:3px solid var(--accent)}
.cbwrap{display:grid;gap:6px;background:var(--card);border:1px solid var(--line);border-radius:12px;padding:14px 16px}
.cb{display:grid;grid-template-columns:96px 1fr 62px;gap:10px;align-items:center;font-size:13px}
.cn{color:#d7e6ea}
.cbar{height:9px;background:#1d282e;border-radius:5px;overflow:hidden}
.cbar i{display:block;height:100%;background:linear-gradient(90deg,#4c9ba1,#7fd3c6)}
.cv{text-align:right;color:#bfe6ea;font-variant-numeric:tabular-nums}
.gcard{background:var(--card);border:1px solid var(--line);border-radius:12px;margin-bottom:14px;overflow:hidden}
.ghead{padding:10px 14px;background:var(--card2);font-size:13.5px;font-weight:600;border-bottom:1px solid var(--line)}
table{border-collapse:collapse;width:100%;font-size:13px}
th,td{padding:9px 10px;text-align:left;border-bottom:1px solid var(--line);white-space:nowrap}
th{color:var(--dim);font-weight:500;font-size:12.5px}
.gt tbody tr:last-child td,.gt tbody tr:last-child th{border-bottom:0}
.tm{font-weight:550}
.elo{color:#6f858d;font-size:11.5px;margin-left:6px;font-variant-numeric:tabular-nums}
.num{text-align:right;font-variant-numeric:tabular-nums}
.ch{color:#7fd3c6;font-weight:600}
.qf{color:#bfe6ea}
.mini{display:grid;grid-template-columns:repeat(4,1fr);gap:6px;min-width:240px}
.px{display:flex;flex-direction:column;gap:2px}
.pk{color:#6f858d;font-size:10.5px}
.pv{font-size:12px;font-variant-numeric:tabular-nums}
.fixt{background:var(--card);border:1px solid var(--line);border-radius:12px;overflow-x:auto}
.mono{font-variant-numeric:tabular-nums;color:var(--dim)}
.vs{color:#6f858d;padding:0 2px}
.sc{display:inline-block;margin-right:8px;padding:2px 8px;border-radius:6px;background:#1d282e;font-variant-numeric:tabular-nums;font-size:12.5px}
.sc b{color:#7fd3c6;font-weight:600}
.sc.big{font-size:13.5px;padding:5px 11px}
.fsbox{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:14px 16px}
.sensbox{display:grid;gap:10px}
.scard{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:12px 16px;font-size:13px}
.scard b{color:#cfe3e8;display:block;margin-bottom:4px}
.scard em{color:#7fd3c6;font-style:normal;font-weight:600}
.assump{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:14px 16px 14px 34px;color:var(--dim);font-size:12.8px}
.assump li{margin:5px 0}
footer{margin-top:30px;color:#6f858d;font-size:12px;text-align:center;line-height:1.8}
</style>
</head>
<body><div class="wrap">
<div class="hero">
  <h1>${esc(T.name)} ${esc(T.season)} · 盃賽模擬</h1>
  <div class="sub">Monte Carlo ${T.sims.toLocaleString()} 次（seed ${T.seed}）· 強度池 ${T.model.n} 場 / ${T.model.teams} 隊 · 主場係數 ×${T.model.H} · 每隊基準 ${T.model.base} 球</div>
  <div class="gen">產生時間 ${esc(T.generatedAt)} · 已完場分組賽用真實賽果，未踢場次由模型抽樣</div>
</div>

<div class="method">
  <div class="mcard"><b>① 隊力合成</b><span>Elo 0.60（時序 K=25）＋攻防擬合 0.25＋近況 0.10（近 365 日場均積分）＋年齡 0.05（偏離 26.5 歲高峰）。教練／戰術與定性殘差冇可靠數據源，設 0。</span></div>
  <div class="mcard"><b>② 逐場模擬</b><span>Step 1 的 Dixon-Coles 泊松（ρ=−0.08）抽樣入球；合成強度以 λ 比值 exp(0.08×Δ) 微調，總入球水平不變。</span></div>
  <div class="mcard"><b>③ 賽制</b><span>分組前 2 出線八強；種子（首名）對非種子（次名），同組回避，兩回合、種子隊次回合主場；四強與決賽中立場。</span></div>
  <div class="mcard"><b>④ 限制</b><span>和局以十二碼 50/50 決勝；分組同分比淨勝球再比入球，最後隨機；模型難以穩定贏市場。</span></div>
</div>
<div class="warn">本頁是統計模擬，<b>不構成任何投注建議</b>。機率只反映模型假設下的相對強弱，唔代表真實結果；市場賠率從未參與運算。</div>
${missing}
<div class="sect-title">奪冠機率（前十）</div>
<div class="cbwrap">${champBars}</div>

<div class="sect-title">分組形勢（名次機率 · 出線 · 晉級 · 奪冠）</div>
${groupHtml}

<div class="sect-title">未踢場次 · 最可能比分（前 3）</div>
<div class="fixt"><table><thead><tr><th>開賽（北京時間）</th><th>組</th><th>對賽</th><th>典型比分</th></tr></thead><tbody>${fixtureRows}</tbody></table></div>
${finalScores}
${sensHtml}
<div class="sect-title">模型假設（全部可查）</div>
<ol class="assump">${assumptions}</ol>
<footer>Step 3 盃賽模擬 · 模型輸出非投注建議 · 數據源 FotMob（賽制）＋自建強度池（725 場國際賽）</footer>
</div></body></html>`;

fs.mkdirSync(path.dirname(OUT), { recursive: true });
fs.writeFileSync(OUT, HTML);
console.log(`已寫入 ${path.relative(HERE, OUT)}（${HTML.length.toLocaleString()} bytes，${T.teams.length} 隊 / ${T.fixtures.length} 場未踢）`);