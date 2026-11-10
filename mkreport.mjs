#!/usr/bin/env node
/**
 * mkreport.mjs — 用 report-template.html 產出「足球賽事預測分析報告」HTML（SKILL.md L186-242）
 *
 *   node mkreport.mjs                     # 用 fixtures.json 內所有未開賽/進行中有預測的場次
 *   node mkreport.mjs --in api-fixtures.json --out dist/report.html
 *   node mkreport.mjs --limit 20          # 只取前 N 場（示範用）
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const argv = process.argv.slice(2);
const opt = (k, d) => { const i = argv.indexOf('--' + k); return i < 0 ? d : (argv[i + 1] ?? true); };

const IN = path.join(HERE, opt('in', 'fixtures.json'));
const OUT = path.join(HERE, opt('out', 'dist/report.html'));
const TPL = path.join(HERE, 'report-template.html');
const LIMIT = Number(opt('limit', 0));

const fx = JSON.parse(fs.readFileSync(IN, 'utf8'));
const rows = (fx.rows || []).filter((r) => r.pred && (r.status === 'pre' || r.status === 'live'));

/** 讓球顯示：用 ahd 主盤（|cover-0.5| 最小嗰檔）＋讓球方，寫成「主 -1.25」 */
function hcpText(p) {
  const ahd = p.ahd;
  if (!ahd || !ahd.mark) return null;
  const m = Math.abs(Number(ahd.mark));
  const row = (ahd.rows || []).find((r) => Math.abs(Math.abs(r.line) - m) < 1e-9);
  const fav = (row && row.fav) || 'h';
  return (fav === 'h' ? '主 -' : '客 -') + m;
}

const pad = (n) => String(n).padStart(2, '0');
const hk = (iso) => {
  const d = new Date(iso);
  if (isNaN(d)) return String(iso || '');
  // 固定 +08:00（報告統一北京時間）
  const t = new Date(d.getTime() + 8 * 3600_000);
  return `${pad(t.getUTCMonth() + 1)}-${pad(t.getUTCDate())} ${pad(t.getUTCHours())}:${pad(t.getUTCMinutes())}`;
};

/** 因子：模板要求 {p:視角, txt:論述, src, url}；本管線的 p 是論述句，故按內容歸類視角 */
function factorOf(f) {
  if (typeof f === 'string') return { p: '情境', txt: f };
  const all = String(f.p || '') + ' ' + String(f.txt || '');
  let p = '統計';
  if (/傷停|缺陣|風險|輪換|背靠背|密程|停賽/.test(all)) p = '風險';
  else if (/Elo|攻防|λ|xG|泊松|樣本/.test(all)) p = '統計';
  else if (/共識層|代理/.test(all)) p = '共識';
  else if (/控球|戰術|陣式|逼搶/.test(all)) p = '戰術';
  else if (/休息|旅|主場|動機|賽程/.test(all)) p = '情境';
  return { p, txt: [f.p, f.txt].filter(Boolean).join('｜'), src: f.src || '', url: f.url || '' };
}

/** 風險清單：全部由既有診斷欄位推導，唔憑空寫（SKILL L226 fin.risks） */
function risksOf(r) {
  const p = r.pred, dg = p.diag || {}, out = [];
  const n = Array.isArray(dg.seasons) ? dg.seasons.reduce((a, s) => a + (s[1] || 0), 0) : (dg.n || 0);
  if (n && n < 300) out.push(`該聯賽歷史樣本僅 ${n} 場，攻防與 λ 估計誤差較大`);
  const lineups = String(dg.lineupState || '');
  if (!/official/i.test(lineups)) out.push('未取得官方首發，陣容以近期主力推測（假設主力齊整）');
  const used = (dg.consensus || []).filter((a) => a.used);
  if (dg.consensus && used.length < 4) {
    out.push(`共識層僅 ${used.length}/4 代理取得可用數據（${(dg.consensus || []).filter((a) => !a.used).map((a) => a.name).join('、') || '—'} 未生效）`);
  }
  if (!p.cal) out.push('此聯賽無同分布校準器，只提供基線（無 cal 列）');
  if (r.status === 'live') out.push('比賽進行中，本預測為賽前版本，未計入場上比分');
  if ((p.fin && p.fin.cs) <= 2) out.push('模型自評信心偏低，任何一路結果皆有可能');
  return out;
}

const DATA = rows.slice(0, LIMIT > 0 ? LIMIT : rows.length).map((r, i) => {
  const p = r.pred, dg = p.diag || {};
  const fin = p.fin || {}, base = p.base || {}, reb = p.reb || base;
  const tops = ((dg.tops || []).slice(0, 3)).map((t) => ({ s: String(t.s), p: +Number(t.p).toFixed(4) }));
  const k = ['h', 'd', 'a'].reduce((x, y) => ((fin[y] || 0) > (fin[x] || 0) ? y : x), 'h');
  const mlText = `${k === 'h' ? '主勝 ' + r.home : k === 'd' ? '和局' : '客勝 ' + r.away}（${(fin[k] * 100).toFixed(1)}%）`;
  return {
    m_id: String(i + 1),
    home: r.home,
    away: r.away,
    comp: r.compName || r.comp,
    kickoff: hk(r.kick),
    handicap: hcpText(p),
    base: {
      h: +Number(base.h || 0).toFixed(4),
      d: +Number(base.d || 0).toFixed(4),
      a: +Number(base.a || 0).toFixed(4),
      lh: +Number(base.lh || 0).toFixed(3),
      la: +Number(base.la || 0).toFixed(3),
      sl: tops,
    },
    factors: (p.factors || []).map(factorOf),
    reb: { h: +Number(reb.h || 0).toFixed(4), d: +Number(reb.d || 0).toFixed(4), a: +Number(reb.a || 0).toFixed(4) },
    fin: {
      h: +Number(fin.h || 0).toFixed(4),
      d: +Number(fin.d || 0).toFixed(4),
      a: +Number(fin.a || 0).toFixed(4),
      ml_text: mlText,
      conf: fin.conf || '★★★☆☆',
      cs: fin.cs || 3,
      risks: (fin.risks && fin.risks.length ? fin.risks : risksOf(r)),
    },
    cal: p.cal && p.cal.h != null ? { h: +p.cal.h, d: +p.cal.d, a: +p.cal.a, note: p.cal.note || '' } : null,
    note: p.note || '',
    lineup: dg.lineupState || '',
    elo: dg.elo ? `${Math.round(dg.elo.home)} vs ${Math.round(dg.elo.away)}` : '',
  };
});

const tpl = fs.readFileSync(TPL, 'utf8');
const stamp = new Date(Date.now() + 8 * 3600_000).toISOString().replace('T', ' ').slice(0, 16) + '（北京時間）';
const html = tpl
  .replace('__TITLE__', `足球賽事預測分析報告 · ${DATA.length} 場`)
  .replace('__SUBTITLE__', `${DATA.length} 場賽事 · 四步流水線模型 · 開球時間見下表`)
  .replace('__GENERATED__', stamp)
  .replace('__DATA__', JSON.stringify(DATA, null, 0));

fs.mkdirSync(path.dirname(OUT), { recursive: true });
fs.writeFileSync(OUT, html);
console.log(`報告已生成：${path.relative(HERE, OUT)}（${DATA.length} 場，${(html.length / 1024).toFixed(0)} KB）`);
