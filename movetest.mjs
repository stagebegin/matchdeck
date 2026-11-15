// movetest.mjs — 直接執行 src/main.rs 裏面真正嘅 placeCard／ensureGroup／mdOf 代碼，
// 用一個微型 DOM 驗證「狀態變咗會唔會真係搬卡去正確分頁」。
// 唔引入任何依賴（環境冇 jsdom/linkedom）。publish.sh 以 exit 9 攔截。
import fs from 'node:fs';

class ClassList extends Set { contains(v){ return this.has(v); } toggle(v,f){ if(f===undefined){ this.has(v)?this.delete(v):this.add(v); } else if(f) this.add(v); else this.delete(v); return this.has(v); } }

class El {
  constructor(tag) {
    this.tagName = String(tag || 'div').toLowerCase();
    this.children = [];
    this.parent = null;
    this._id = '';
    this.classList = new ClassList();
    this.dataset = {};
    this._text = '';
  }
  get id() { return this._id; }
  set id(v) { this._id = String(v); }
  get className() { return [...this.classList].join(' '); }
  set className(v) { this.classList = new ClassList(String(v).split(/\s+/).filter(Boolean)); }
  get firstElementChild() { return this.children[0] || null; }
  get nextElementSibling() {
    if (!this.parent) return null;
    const i = this.parent.children.indexOf(this);
    return i < 0 ? null : this.parent.children[i + 1] || null;
  }
  appendChild(c) {
    if (c.parent) { const i = c.parent.children.indexOf(c); if (i >= 0) c.parent.children.splice(i, 1); }
    c.parent = this; this.children.push(c); return c;
  }
  insertBefore(c, ref) {
    if (c.parent) { const i = c.parent.children.indexOf(c); if (i >= 0) c.parent.children.splice(i, 1); }
    c.parent = this;
    const i = this.children.indexOf(ref);
    if (i < 0) this.children.push(c); else this.children.splice(i, 0, c);
    return c;
  }
  remove() {
    if (!this.parent) return;
    const i = this.parent.children.indexOf(this);
    if (i >= 0) this.parent.children.splice(i, 1);
    this.parent = null;
  }
  get textContent() { return this._text + this.children.map((c) => c.textContent).join(''); }
  set textContent(v) { this.children = []; this._text = String(v); }
  _sel(sel) {
    // 支援 tag / .a.b / #id / [data-x="v"] 及其組合
    const parts = String(sel).trim();
    let tag = null, id = null;
    const cls = [], attrs = [];
    let rest = parts;
    const tagM = rest.match(/^[a-zA-Z][\w-]*/);
    if (tagM) { tag = tagM[0].toLowerCase(); rest = rest.slice(tagM[0].length); }
    for (const m of rest.matchAll(/#([\w-]+)/g)) id = m[1];
    for (const m of rest.matchAll(/\.([\w-]+)/g)) cls.push(m[1]);
    for (const m of rest.matchAll(/\[data-([\w-]+)="([^"]*)"\]/g)) attrs.push([m[1], m[2]]);
    return (el) => {
      if (tag && el.tagName !== tag) return false;
      if (id && el.id !== id) return false;
      for (const c of cls) if (!el.classList.has(c)) return false;
      for (const [k, v] of attrs) if (String(el.dataset[k] ?? '') !== v) return false;
      return true;
    };
  }
  _descendants(out = []) { for (const c of this.children) { out.push(c); c._descendants(out); } return out; }
  querySelectorAll(sel) { const f = this._sel(sel); return this._descendants().filter(f); }
  querySelector(sel) { const f = this._sel(sel); return this._descendants().find(f) || null; }
  set innerHTML(html) {
    // 只解析 ensureGroup 實際會產生嘅兩段：dayhead + grid
    this.children = [];
    const m = String(html).match(/<div class="dayhead" id="([^"]*)"><b>([^<]*)<\/b><span>([^<]*)<\/span><\/div><div class="grid"><\/div>/);
    if (!m) throw new Error('movetest: 唔認得 ensureGroup 產生嘅 HTML: ' + html);
    const head = new El('div'); head.className = 'dayhead'; head.id = m[1];
    const b = new El('b'); b.textContent = m[2];
    const sp = new El('span'); sp.textContent = m[3];
    head.appendChild(b); head.appendChild(sp);
    const grid = new El('div'); grid.className = 'grid';
    this.appendChild(head); this.appendChild(grid);
  }
}

// 由 src/main.rs 抽真正嘅函數體（唔複製，避免同源碼漂移）
const src = fs.readFileSync(new URL('./src/main.rs', import.meta.url), 'utf8');
const from = src.indexOf('function mdOf(');
const to = src.indexOf('function toast(');
if (from < 0 || to < 0 || to < from) { console.error('✗ movetest: 找唔到 mdOf／toast 標記，源碼結構變咗要更新測試'); process.exit(9); }
const code = src.slice(from, to);
const WD = ['日', '一', '二', '三', '四', '五', '六'];
const document = { createElement: (t) => new El(t) };
const fns = new Function('document', 'WD', code + '\nreturn {mdOf,labelOf,ensureGroup,placeCard};')(document, WD);
const { mdOf, placeCard } = fns;

const problems = [];
const card = (id) => { const c = new El('div'); c.className = 'card'; c.dataset.id = id; return c; };
const section = (name) => { const s = new El('section'); s.className = 'tabbody'; s.dataset.body = name; return s; };

// ① 空嘅「比賽中」分頁（只有 .panel.empty）→ 搬一張卡入去
{
  const live = section('live');
  const empty = new El('div'); empty.className = 'panel empty'; live.appendChild(empty);
  placeCard(live, card('A1'), '2026-11-16T21:00:00+08:00');
  const heads = live.querySelectorAll('.dayhead');
  if (heads.length !== 1) problems.push(`① 應該只有一個日期組，實際 ${heads.length}`);
  if (heads[0] && heads[0].id !== 'day-11-16') problems.push(`① 日期組 id 錯：${heads[0] && heads[0].id}`);
  if (live.querySelector('.panel.empty')) problems.push('① .panel.empty 冇移除（會顯示「今日冇比賽」）');
  const g = heads[0] && heads[0].nextElementSibling;
  if (!g || g.classList.contains('grid') === false) problems.push('① dayhead 後面唔係 .grid');
  if (g && g.querySelectorAll('.card').length !== 1) problems.push('① grid 內卡數唔係 1');
  const sp = heads[0] && heads[0].querySelector('span');
  if (!sp || sp.textContent !== '1 場') problems.push(`① 場數標籤錯：${sp && sp.textContent}`);
  const lbl = heads[0] && heads[0].querySelector('b');
  if (!lbl || !/^\d\d\/\d\d 週[日一二三四五六]$/.test(lbl.textContent)) problems.push(`① 標籤格式錯：${lbl && lbl.textContent}`);
}

// ② 同一日再搬一張 → 併入同一組、場數 2、唔會重複建組
{
  const live = section('live');
  placeCard(live, card('B1'), '2026-11-16T19:30:00+08:00');
  placeCard(live, card('B2'), '2026-11-16T21:00:00+08:00');
  const heads = live.querySelectorAll('.dayhead');
  if (heads.length !== 1) problems.push(`② 同一天應該合併成一組，實際 ${heads.length} 組`);
  if (heads[0] && heads[0].querySelector('span').textContent !== '2 場') problems.push(`② 場數應為「2 場」，實際「${heads[0] && heads[0].querySelector('span').textContent}」`);
  const order = heads[0] && heads[0].nextElementSibling.children.map((c) => c.dataset.id);
  if (order && order.join(',') !== 'B1,B2') problems.push(`② 卡次序錯：${order.join(',')}`);
}

// ③ 亂序搬入三日 → 日期組要按日子排（唔可以接喺尾）
{
  const live = section('live');
  placeCard(live, card('C18'), '2026-11-18T20:00:00+08:00');
  placeCard(live, card('C16'), '2026-11-16T20:00:00+08:00');
  placeCard(live, card('C17'), '2026-11-17T20:00:00+08:00');
  const ids = live.querySelectorAll('.dayhead').map((h) => h.id).join(',');
  if (ids !== 'day-11-16,day-11-17,day-11-18') problems.push(`③ 日期組次序錯：${ids}`);
}

// ④ 有 .daypage 嘅分頁（未開賽分頁）→ 卡要入對應日嘅 daypage
{
  const pre = section('pre');
  for (const d of ['day-11-16', 'day-11-17']) {
    const pg = new El('div'); pg.className = 'daypage'; pg.dataset.day = d; pre.appendChild(pg);
  }
  placeCard(pre, card('D1'), '2026-11-17T20:00:00+08:00');
  const pg17 = pre.querySelector('.daypage[data-day="day-11-17"]');
  if (!pg17 || pg17.querySelectorAll('.card').length !== 1) problems.push('④ 卡冇入到 11/17 嘅 daypage');
  const pg16 = pre.querySelector('.daypage[data-day="day-11-16"]');
  if (pg16 && pg16.querySelectorAll('.card').length !== 0) problems.push('④ 卡入錯咗 11/16 嘅 daypage');
}

// ⑤ 冇 kickoff（API 缺欄）→ 用今日做退路，唔可以拋錯
{
  const live = section('live');
  try { placeCard(live, card('E1'), ''); } catch (e) { problems.push('⑤ 空 kickoff 拋錯：' + e.message); }
  if (!live.querySelectorAll('.dayhead').length) problems.push('⑤ 空 kickoff 冇建立任何日期組');
}

if (problems.length) {
  console.error('✗ 搬卡邏輯有問題：');
  for (const p of problems) console.error('  - ' + p);
  process.exit(9);
}
console.log('✓ 搬卡邏輯（placeCard／ensureGroup）5 項測試全部通過');
