//! matchdeck — 靜態賽前預測看板生成器
//!
//! 讀 `fb/preds/*.json`（skill DATA schema）＋ `registry.json`，
//! 產出單一 `index.html`（霧青色主題、零外部資源、可直接放 GitHub Pages）。
//!
//! cargo run --release -- --preds ../../fb/preds --registry ../../fb/registry.json --out dist

mod json;
use crate::json::J as Value;
use std::fs;
use std::path::{Path, PathBuf};

// ---------- 取值小工具 ----------

fn get<'a>(v: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cur = v;
    for k in path {
        cur = cur.get(*k)?;
    }
    Some(cur)
}

fn num(v: &Value, path: &[&str], default: f64) -> f64 {
    get(v, path).and_then(|x| x.as_f64()).unwrap_or(default)
}

fn text(v: &Value, path: &[&str]) -> String {
    match get(v, path) {
        Some(Value::Str(s)) => s.clone(),
        Some(Value::Num(n)) => json::num_str(*n),
        Some(Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

fn arr_len(v: &Value, path: &[&str]) -> usize {
    get(v, path).and_then(|x| x.as_arr()).map(|a| a.len()).unwrap_or(0)
}

fn pct(x: f64) -> String {
    format!("{:.1}%", x * 100.0)
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            _ => o.push(c),
        }
    }
    o
}

/// "2026-11-07T20:30:00.000+08:00" -> ("11-07", "20:30")
fn kick_parts(iso: &str) -> (String, String) {
    if let Some(tpos) = iso.find('T') {
        let date = &iso[..tpos];
        let time = &iso[tpos + 1..std::cmp::min(tpos + 6, iso.len())];
        let md = if date.len() >= 10 { &date[5..10] } else { date };
        return (md.to_string(), time.to_string());
    }
    (iso.to_string(), String::new())
}

fn weekday(md: &str) -> &'static str {
    // Zeller-ish：只為顯示用，年份固定 2026
    let (m, d) = match (md.get(0..2), md.get(3..5)) {
        (Some(m), Some(d)) => (m.parse::<i32>().unwrap_or(1), d.parse::<i32>().unwrap_or(1)),
        _ => return "",
    };
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = 2026i32;
    let w = (y + y / 4 - y / 100 + y / 400 + t[(m - 1) as usize] + d) % 7;
    ["日", "一", "二", "三", "四", "五", "六"][w as usize]
}

// ---------- 卡片 ----------

fn card(p: &Value) -> String {
    let comp = text(p, &["comp"]);
    let comp_name = text(p, &["compName"]);
    let home = text(p, &["home"]);
    let away = text(p, &["away"]);
    let kick = text(p, &["kickoff"]);
    let (md, hm) = kick_parts(&kick);
    let pass = text(p, &["pass"]);
    let pass_zh = match pass.as_str() {
        "t60" => "T-60",
        "t25" => "T-25",
        _ => "手動",
    };

    let bh = num(p, &["base", "h"], 0.0) * 100.0;
    let bd = num(p, &["base", "d"], 0.0) * 100.0;
    let ba = num(p, &["base", "a"], 0.0) * 100.0;
    let fh = num(p, &["fin", "h"], 0.0);
    let fd = num(p, &["fin", "d"], 0.0);
    let fa = num(p, &["fin", "a"], 0.0);

    let lh = num(p, &["base", "lh"], 0.0);
    let la = num(p, &["base", "la"], 0.0);
    let lh2 = num(p, &["diag", "lambda_adj", "lh"], lh);
    let la2 = num(p, &["diag", "lambda_adj", "la"], la);

    let elo_h = num(p, &["diag", "elo", "home"], 1500.0);
    let elo_a = num(p, &["diag", "elo", "away"], 1500.0);
    let o25 = num(p, &["diag", "o25"], 0.0);
    let o35 = num(p, &["diag", "o35"], 0.0);
    let btts = num(p, &["diag", "btts"], 0.0);
    let n = num(p, &["diag", "n"], 0.0);
    let conf = text(p, &["fin", "conf"]);
    let cs = num(p, &["fin", "cs"], 3.0);
    let lineup = text(p, &["diag", "lineupState"]);
    let inj_h = arr_len(p, &["diag", "injuries", "home"]);
    let inj_a = arr_len(p, &["diag", "injuries", "away"]);

    let hc = |line: &str, k: &str| num(p, &["diag", "handicap", line, k], 0.0);
    let (m1h, m1d, m1a) = (hc("-1", "h"), hc("-1", "d"), hc("-1", "a"));
    let (p1h, p1d, p1a) = (hc("1", "h"), hc("1", "d"), hc("1", "a"));

    let tops: Vec<String> = get(p, &["diag", "tops"])
        .and_then(|x| x.as_arr())
        .map(|a| {
            a.iter()
                .take(4)
                .filter_map(|t| {
                    let s = text(t, &["s"]);
                    let pr = num(t, &["p"], 0.0);
                    if s.is_empty() { None } else { Some(format!("<span class=\"chip\"><b>{}</b><i>{}</i></span>", esc(&s), pct(pr))) }
                })
                .collect()
        })
        .unwrap_or_default();

    let inj_chip = if inj_h + inj_a > 0 {
        format!("<span class=\"tag\">傷停 {} / {}</span>", inj_h, inj_a)
    } else {
        "<span class=\"tag\">傷停 —</span>".to_string()
    };
    let lineup_cls = if lineup.starts_with('✅') { "ok" } else if lineup.starts_with('◐') { "mid" } else { "low" };

    let wd = weekday(&md);
    format!(
        r#"<article class="card" data-comp="{comp}" data-text="{plain}">
  <div class="card-top">
    <span class="badge">{comp}</span>
    <span class="comp-name">{comp_name}</span>
    <span class="kick">{md} 週{wd} {hm}</span>
    <span class="pass">{pass_zh}</span>
  </div>
  <div class="teams">
    <div class="team home"><span class="name">{home}</span><span class="elo">Elo {elo_h:.0}</span></div>
    <div class="vs">vs</div>
    <div class="team away"><span class="name">{away}</span><span class="elo">Elo {elo_a:.0}</span></div>
  </div>
  <div class="bar" role="img" aria-label="主 {fh_p} 和 {fd_p} 客 {fa_p}">
    <span class="seg h" style="width:{bh_p}%" title="主 {bh_p}"></span>
    <span class="seg d" style="width:{bd_p}%" title="和 {bd_p}"></span>
    <span class="seg a" style="width:{ba_p}%" title="客 {ba_p}"></span>
  </div>
  <div class="nums">
    <div class="n h"><b>{fh_p}</b><i>主勝</i></div>
    <div class="n d"><b>{fd_p}</b><i>和</i></div>
    <div class="n a"><b>{fa_p}</b><i>客勝</i></div>
  </div>
  <div class="meta">
    <span>λ {lh2:.2} / {la2:.2}</span>
    <span>大 2.5 {o25_p}</span>
    <span>大 3.5 {o35_p}</span>
    <span>BTTS {btts_p}</span>
    <span>樣本 {n:.0}</span>
  </div>
  <div class="hc">
    <div class="hc-row"><span class="hc-key">讓球 −1</span><span>{m1h_p}</span><span>{m1d_p}</span><span>{m1a_p}</span></div>
    <div class="hc-row"><span class="hc-key">讓球 +1</span><span>{p1h_p}</span><span>{p1d_p}</span><span>{p1a_p}</span></div>
  </div>
  <div class="tops">{tops}</div>
  <div class="foot">
    <span class="conf" data-cs="{cs:.0}">{conf}</span>
    <span class="tag {lineup_cls}">首發 {lineup}</span>
    {inj_chip}
  </div>
</article>"#,
        comp = esc(&comp),
        comp_name = esc(&comp_name),
        home = esc(&home),
        away = esc(&away),
        plain = esc(&format!("{} {} {} {}", comp, home, away, comp_name)),
        md = md,
        wd = wd,
        hm = hm,
        pass_zh = pass_zh,
        elo_h = elo_h,
        elo_a = elo_a,
        bh_p = format!("{:.1}", bh),
        bd_p = format!("{:.1}", bd),
        ba_p = format!("{:.1}", ba),
        fh_p = pct(fh),
        fd_p = pct(fd),
        fa_p = pct(fa),
        lh2 = lh2,
        la2 = la2,
        o25_p = pct(o25),
        o35_p = pct(o35),
        btts_p = pct(btts),
        n = n,
        m1h_p = pct(m1h),
        m1d_p = pct(m1d),
        m1a_p = pct(m1a),
        p1h_p = pct(p1h),
        p1d_p = pct(p1d),
        p1a_p = pct(p1a),
        tops = tops.join(""),
        conf = esc(&conf),
        cs = cs,
        lineup = esc(&lineup),
        inj_chip = inj_chip,
    )
}

// ---------- 頁面 ----------

const CSS: &str = r#"
:root{
  --mist-0:#f5fbfb; --mist-1:#e8f3f3; --mist-2:#d9ecec; --mist-3:#c8e3e4;
  --ink:#15302f; --ink-2:#3d5a5a; --muted:#6f8c8d;
  --cyan:#4c9ba1; --cyan-2:#7cb8bc; --cyan-3:#a6cfd1;
  --home:#3f8f95; --draw:#9db6b8; --away:#2b616a;
  --card:rgba(255,255,255,.72); --line:rgba(21,48,47,.09);
  --radius:18px;
}
*{box-sizing:border-box}
html{scroll-behavior:smooth}
body{
  margin:0; color:var(--ink);
  font-family:"Noto Sans TC","PingFang TC","Hiragino Sans HK","Microsoft JhengHei",system-ui,-apple-system,"Segoe UI",sans-serif;
  background:
    radial-gradient(1100px 520px at 12% -8%, #ffffff 0%, rgba(255,255,255,0) 60%),
    radial-gradient(900px 480px at 92% 4%, var(--mist-2) 0%, rgba(217,236,236,0) 62%),
    linear-gradient(180deg,var(--mist-0) 0%,var(--mist-1) 38%,var(--mist-2) 100%);
  background-attachment:fixed;
  -webkit-font-smoothing:antialiased;
}
.wrap{max-width:1180px;margin:0 auto;padding:0 22px 72px}

/* hero */
.hero{position:relative;padding:64px 0 30px;overflow:hidden}
.hero .mist{position:absolute;inset:-40% -10% auto -10%;height:340px;
  background:radial-gradient(closest-side,rgba(124,184,188,.34),transparent 70%),
             radial-gradient(closest-side at 70% 30%,rgba(166,207,209,.45),transparent 72%);
  filter:blur(6px);pointer-events:none}
h1{margin:0;font-size:clamp(30px,4.2vw,46px);letter-spacing:.02em;font-weight:700;position:relative}
h1 .dot{display:inline-block;width:.42em;height:.42em;border-radius:50%;margin-left:.18em;
  background:var(--cyan);box-shadow:0 0 0 6px rgba(76,155,161,.14);vertical-align:.08em}
.tagline{margin:12px 0 0;color:var(--ink-2);font-size:15px;letter-spacing:.03em}
.tagline b{color:var(--cyan);font-weight:600}
.stats{display:flex;flex-wrap:wrap;gap:10px;margin-top:26px;position:relative}
.stat{background:var(--card);backdrop-filter:blur(10px);border:1px solid var(--line);
  border-radius:14px;padding:12px 16px;min-width:128px}
.stat b{display:block;font-size:22px;letter-spacing:.02em}
.stat i{display:block;font-style:normal;color:var(--muted);font-size:12px;margin-top:2px;letter-spacing:.06em}

/* toolbar */
.toolbar{position:sticky;top:0;z-index:20;display:flex;gap:10px;align-items:center;flex-wrap:wrap;
  padding:14px 0;margin:8px 0 18px;
  background:linear-gradient(180deg,rgba(236,246,246,.96),rgba(236,246,246,.80));
  backdrop-filter:blur(12px);border-bottom:1px solid var(--line)}
.search{flex:1 1 220px;min-width:180px;padding:9px 14px;border-radius:12px;border:1px solid var(--line);
  background:rgba(255,255,255,.86);color:var(--ink);font-size:14px;outline:none}
.search:focus{border-color:var(--cyan-2);box-shadow:0 0 0 3px rgba(124,184,188,.18)}
.chips{display:flex;gap:6px;flex-wrap:wrap}
.fchip{cursor:pointer;border:1px solid var(--line);background:rgba(255,255,255,.7);color:var(--ink-2);
  padding:6px 11px;border-radius:999px;font-size:12.5px;letter-spacing:.04em;transition:.18s}
.fchip:hover{border-color:var(--cyan-2)}
.fchip.on{background:var(--cyan);color:#fff;border-color:var(--cyan)}

/* grid */
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(342px,1fr));gap:18px}

/* card */
.card{position:relative;background:var(--card);border:1px solid var(--line);border-radius:var(--radius);
  padding:18px 18px 15px;backdrop-filter:blur(14px);
  box-shadow:0 1px 0 rgba(255,255,255,.7) inset,0 10px 30px -18px rgba(21,48,47,.35);
  overflow:hidden}
.card::after{content:"";position:absolute;inset:0;pointer-events:none;
  background:radial-gradient(240px 120px at 100% 0%,rgba(166,207,209,.28),transparent 70%)}
.card-top{display:flex;align-items:center;gap:8px;font-size:12px;color:var(--muted);position:relative}
.badge{background:var(--cyan);color:#fff;border-radius:7px;padding:2px 7px;font-size:11.5px;letter-spacing:.08em;font-weight:600}
.comp-name{white-space:nowrap;overflow:hidden;text-overflow:ellipsis;max-width:38%}
.kick{margin-left:auto;font-variant-numeric:tabular-nums;color:var(--ink-2)}
.pass{border:1px solid var(--cyan-3);color:var(--cyan);border-radius:6px;padding:1px 6px;font-size:11px}

.teams{display:flex;align-items:center;gap:10px;margin:16px 0 14px;position:relative}
.team{flex:1;display:flex;flex-direction:column;gap:3px;min-width:0}
.team.away{align-items:flex-end;text-align:right}
.team .name{font-size:17.5px;font-weight:650;letter-spacing:.01em;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.team .elo{font-size:11.5px;color:var(--muted);font-variant-numeric:tabular-nums}
.vs{color:var(--muted);font-size:12px;letter-spacing:.1em}

.bar{display:flex;height:9px;border-radius:999px;overflow:hidden;background:var(--mist-2);position:relative}
.bar .seg{display:block;height:100%}
.bar .h{background:var(--home)} .bar .d{background:var(--draw)} .bar .a{background:var(--away)}

.nums{display:grid;grid-template-columns:repeat(3,1fr);margin-top:10px;text-align:center}
.nums .n b{display:block;font-size:19px;font-variant-numeric:tabular-nums;letter-spacing:.01em}
.nums .n i{display:block;font-style:normal;font-size:11.5px;color:var(--muted);margin-top:1px}
.nums .h b{color:var(--home)} .nums .d b{color:#7f9a9c} .nums .a b{color:var(--away)}

.meta{display:flex;flex-wrap:wrap;gap:6px 12px;margin-top:12px;font-size:12px;color:var(--ink-2);
  font-variant-numeric:tabular-nums}
.meta span{position:relative}

.hc{margin-top:12px;border-top:1px dashed var(--line);padding-top:9px;font-size:12.5px;
  font-variant-numeric:tabular-nums;color:var(--ink-2)}
.hc-row{display:grid;grid-template-columns:78px 1fr 1fr 1fr;gap:4px;padding:2px 0}
.hc-row span{text-align:right}
.hc-key{text-align:left !important;color:var(--muted)}

.tops{display:flex;flex-wrap:wrap;gap:6px;margin-top:11px}
.chip{display:inline-flex;align-items:center;gap:5px;border:1px solid var(--line);
  background:rgba(255,255,255,.6);border-radius:8px;padding:3px 8px;font-size:12px}
.chip b{font-variant-numeric:tabular-nums;letter-spacing:.04em}
.chip i{font-style:normal;color:var(--muted);font-size:11px}

.foot{display:flex;align-items:center;gap:8px;margin-top:13px;padding-top:11px;
  border-top:1px solid var(--line);font-size:11.5px;color:var(--muted)}
.conf{color:#c58b52;letter-spacing:.12em}
.tag{border:1px solid var(--line);border-radius:6px;padding:1px 7px}
.tag.ok{color:#3f8f77;border-color:rgba(63,143,119,.3)}
.tag.mid{color:#a58a3f;border-color:rgba(165,138,63,.3)}
.tag.low{color:var(--muted)}
.foot .conf+.tag{margin-left:0}

/* sections */
section.block{margin-top:46px}
h2{font-size:19px;letter-spacing:.05em;margin:0 0 14px;display:flex;align-items:center;gap:10px}
h2::before{content:"";width:16px;height:2px;background:var(--cyan);display:inline-block}
.panel{background:var(--card);border:1px solid var(--line);border-radius:var(--radius);
  padding:18px 20px;backdrop-filter:blur(12px)}
table{width:100%;border-collapse:collapse;font-size:13.5px}
th,td{text-align:left;padding:8px 6px;border-bottom:1px dashed var(--line)}
th{color:var(--muted);font-weight:500;font-size:12px;letter-spacing:.06em}
tr:last-child td{border-bottom:none}
.st{font-size:11.5px;border-radius:6px;padding:1px 7px;white-space:nowrap}
.st.done{color:#3f8f77;border:1px solid rgba(63,143,119,.3)}
.st.part{color:#a58a3f;border:1px solid rgba(165,138,63,.3)}
.st.todo{color:var(--muted);border:1px solid var(--line)}
.cov{display:flex;flex-wrap:wrap;gap:7px}
.cov span{border:1px solid var(--line);background:rgba(255,255,255,.6);border-radius:8px;
  padding:4px 9px;font-size:12px;color:var(--ink-2)}
.cov span b{color:var(--cyan);font-weight:600}
footer{margin-top:52px;padding-top:18px;border-top:1px solid var(--line);
  color:var(--muted);font-size:12px;line-height:1.9}
footer b{color:var(--ink-2);font-weight:600}
.empty{padding:40px;text-align:center;color:var(--muted)}
@media (max-width:520px){
  .grid{grid-template-columns:1fr}
  .comp-name{display:none}
  .hero{padding-top:44px}
}
"#;

const JS: &str = r#"
(function(){
  const cards=[...document.querySelectorAll('.card')];
  const chips=[...document.querySelectorAll('.fchip')];
  const q=document.getElementById('q');
  let comp='all';
  function apply(){
    const t=(q.value||'').trim().toLowerCase();
    for(const c of cards){
      const okComp = comp==='all' || c.dataset.comp===comp;
      const okText = !t || (c.dataset.text||'').toLowerCase().includes(t);
      c.style.display = okComp&&okText?'':'none';
    }
    const n=cards.filter(c=>c.style.display!=='none').length;
    document.getElementById('shown').textContent=n;
  }
  chips.forEach(ch=>ch.addEventListener('click',()=>{
    chips.forEach(x=>x.classList.remove('on'));
    ch.classList.add('on'); comp=ch.dataset.filter; apply();
  }));
  if(q) q.addEventListener('input',apply);
  apply();
})();
"#;

fn main() {
    let mut args = std::env::args().skip(1);
    let mut preds_dir = PathBuf::from("../../fb/preds");
    let mut registry = PathBuf::from("../../fb/registry.json");
    let mut out = PathBuf::from("dist/index.html");
    while let Some(a) = args.next() {
        match a.as_str() {
            "--preds" => preds_dir = PathBuf::from(args.next().unwrap_or_default()),
            "--registry" => registry = PathBuf::from(args.next().unwrap_or_default()),
            "--out" => out = PathBuf::from(args.next().unwrap_or_default()),
            "-h" | "--help" => {
                println!("matchdeck --preds <dir> --registry <json> --out <html>");
                return;
            }
            _ => {}
        }
    }

    // 讀預測
    let mut preds: Vec<Value> = Vec::new();
    if let Ok(rd) = fs::read_dir(&preds_dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            if let Ok(txt) = fs::read_to_string(&p) {
                if let Ok(v) = json::parse(&txt) {
                    if v.get("home").is_some() && v.get("fin").is_some() {
                        preds.push(v);
                    }
                }
            }
        }
    }
    preds.sort_by(|a, b| text(a, &["kickoff"]).cmp(&text(b, &["kickoff"])));
    let keep_all = std::env::args().any(|a| a == "--all");
    if !keep_all {
        let cut = now_secs() - 90 * 60;
        preds.retain(|p| epoch_secs(&text(p, &["kickoff"])).map(|k| k > cut).unwrap_or(true));
    }

    // 讀 registry
    let reg: Value = fs::read_to_string(&registry)
        .ok()
        .and_then(|t| json::parse(&t).ok())
        .unwrap_or(Value::Null);
    let mut comps: Vec<(String, u64, bool, String)> = Vec::new();
    if let Some(Value::Obj(m)) = reg.get("__root__").cloned().or(Some(reg.clone())) {
        for (k, v) in m {
            if k.starts_with('_') {
                continue;
            }
            let fm = num(&v, &["fotmob"], 0.0) as u64;
            if fm == 0 {
                continue;
            }
            let ver = get(&v, &["verified"]).and_then(|x| x.as_bool()).unwrap_or(false);
            comps.push((k, fm, ver, text(&v, &["name"])));
        }
    }
    comps.sort_by(|a, b| a.0.cmp(&b.0));

    // 每聯賽預測數
    let count = |code: &str| preds.iter().filter(|p| text(p, &["comp"]) == code).count();

    let now = time_now();
    let cards: String = preds.iter().map(card).collect();
    let n_preds = preds.len();
    let n_comps = preds.iter().map(|p| text(p, &["comp"])).collect::<std::collections::BTreeSet<_>>().len();
    let n_verified = comps.iter().filter(|c| c.2).count();

    // 篩選 chips：只列有預測的聯賽
    let mut active: Vec<String> = preds.iter().map(|p| text(p, &["comp"])).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    active.sort();
    let chips = std::iter::once(String::from("<button class=\"fchip on\" data-filter=\"all\">全部</button>"))
        .chain(active.iter().map(|c| format!("<button class=\"fchip\" data-filter=\"{}\">{}</button>", esc(c), esc(c))))
        .collect::<String>();

    let cov: String = comps
        .iter()
        .map(|(c, fm, ver, name)| {
            let n = count(c);
            let v = if *ver { "✓" } else { "·" };
            format!(
                "<span title=\"FotMob league {}\">{} <b>{}</b>{} {}</span>",
                fm,
                esc(c),
                fm,
                v,
                if n > 0 { format!("· {} 場預測", n) } else { esc(name) }
            )
        })
        .collect();

    let rows = [
        ("Step 1 定量基線", "時序 Elo（K=20、主場 +60）＋時間衰減攻防擬合（半衰期 180 天、先驗 K=2）", "done"),
        ("Step 1b 賽果數據", "FotMob 賽果 2023–2026，跨年制聯賽自動處理 2023/2024 編碼", "done"),
        ("Step 1c 傷停／首發", "FotMob squad.injury ＋ matchDetails.lineup（T-60 取預測、T-25 取官方）", "done"),
        ("Step 2 多智能體共識", "四角色微調尚未接上，fin 目前等於基線＋傷停調整", "part"),
        ("Step 3 盃賽 Monte Carlo", "聯賽場景不需要；盃賽待補", "todo"),
        ("Step 4 置信度", "依樣本量／首發狀態／兩路一致性給星", "done"),
        ("Step 5 校準", "需逐聯賽 walk-forward logistic，尚未生成 cal 欄", "todo"),
        ("Step 6 讓球", "負二項 ν=1.4，1X2 仍用泊松", "done"),
    ]
    .iter()
    .map(|(a, b, s)| {
        let (cls, txt) = match *s {
            "done" => ("done", "已完成"),
            "part" => ("part", "部分"),
            _ => ("todo", "未做"),
        };
        format!(
            "<tr><td>{}</td><td>{}</td><td><span class=\"st {}\">{}</span></td></tr>",
            esc(a),
            esc(b),
            cls,
            txt
        )
    })
    .collect::<String>();

    let html = format!(
        r#"<!doctype html>
<html lang="zh-Hant">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>賽前預測台 · 霧青</title>
<meta name="description" content="香港賽馬會賽程 × FotMob 數據 × Elo／泊松模型的賽前預測看板">
<style>{css}</style>
</head>
<body>
<div class="wrap">
  <header class="hero">
    <div class="mist"></div>
    <h1>賽前預測台<span class="dot"></span></h1>
    <p class="tagline">香港賽馬會賽程 <b>×</b> FotMob 賽果與傷停 <b>×</b> Elo＋泊松／負二項</p>
    <div class="stats">
      <div class="stat"><b>{n_preds}</b><i>已出預測</i></div>
      <div class="stat"><b>{n_comps}</b><i>今期聯賽</i></div>
      <div class="stat"><b>{n_verified}</b><i>已驗證聯賽接駁</i></div>
      <div class="stat"><b id="shown">{n_preds}</b><i>目前顯示</i></div>
    </div>
  </header>

  <div class="toolbar">
    <input id="q" class="search" placeholder="搜尋球隊或聯賽…" autocomplete="off">
    <div class="chips">{chips}</div>
  </div>

  <div class="grid">{cards}</div>
  {empty}

  <section class="block">
    <h2>流程合規狀態</h2>
    <div class="panel"><table>
      <thead><tr><th>步驟</th><th>現況</th><th>狀態</th></tr></thead>
      <tbody>{rows}</tbody>
    </table></div>
  </section>

  <section class="block">
    <h2>聯賽接駁</h2>
    <div class="panel"><div class="cov">{cov}</div></div>
  </section>

  <footer>
    <b>生成時間</b>　{now}　·　靜態頁面由 Rust 生成，資料更新後重跑即換。<br>
    <b>免責</b>　本頁為概率模型輸出與數據整理，不構成任何投注建議或誘因。概率為估計值，非保證。<br>
    <b>資料來源</b>　香港賽馬會公開賠率（僅用作選場與賽後對比）· FotMob 賽果／傷停／陣容。
  </footer>
</div>
<script>{js}</script>
</body>
</html>"#,
        css = CSS,
        js = JS,
        n_preds = n_preds,
        n_comps = n_comps,
        n_verified = n_verified,
        chips = chips,
        cards = cards,
        empty = if preds.is_empty() {
            "<div class=\"panel empty\">尚無預測檔。跑 <code>node fb/sched.mjs --once</code> 或等排程觸發。</div>".to_string()
        } else {
            String::new()
        },
        rows = rows,
        cov = cov,
        now = now,
    );

    if let Some(dir) = out.parent() {
        let _ = fs::create_dir_all(dir);
    }
    match fs::write(&out, &html) {
        Ok(_) => println!("wrote {} ({} bytes, {} cards)", out.display(), html.len(), n_preds),
        Err(e) => eprintln!("write failed: {e}"),
    }

    // 同時輸出一份 JSON 給前端／其他工具
    let data_path = out.parent().unwrap_or(Path::new(".")).join("data.json");
    let mut flat = String::from("[\n");
    for (i, p) in preds.iter().enumerate() {
        let rec = Value::Obj(vec![
            ("id".into(), Value::Str(text(p, &["m_id"]))),
            ("comp".into(), Value::Str(text(p, &["comp"]))),
            ("kickoff".into(), Value::Str(text(p, &["kickoff"]))),
            ("home".into(), Value::Str(text(p, &["home"]))),
            ("away".into(), Value::Str(text(p, &["away"]))),
            ("base".into(), p.get("base").cloned().unwrap_or(Value::Null)),
            ("fin".into(), p.get("fin").cloned().unwrap_or(Value::Null)),
            ("handicap".into(), get(p, &["diag", "handicap"]).cloned().unwrap_or(Value::Null)),
            ("lambda".into(), get(p, &["diag", "lambda_adj"]).cloned().unwrap_or(Value::Null)),
            ("lineup".into(), Value::Str(text(p, &["diag", "lineupState"]))),
        ]);
        flat.push_str("  ");
        flat.push_str(&rec.to_string());
        if i + 1 < preds.len() {
            flat.push(',');
        }
        flat.push('\n');
    }
    flat.push(']');
    let _ = fs::write(&data_path, flat);
    println!("wrote {}", data_path.display());
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as i64;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// "2026-11-07T20:30:00.000+08:00" → epoch 秒（只支援 +08:00 或 Z）
fn epoch_secs(iso: &str) -> Option<i64> {
    let t = iso.find('T')?;
    let date = &iso[..t];
    let mut it = date.split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: i64 = it.next()?.parse().ok()?;
    let d: i64 = it.next()?.parse().ok()?;
    let rest = &iso[t + 1..];
    let hh: i64 = rest.get(0..2)?.parse().ok()?;
    let mm: i64 = rest.get(3..5)?.parse().ok()?;
    let off: i64 = if iso.ends_with("+08:00") { 8 * 3600 } else { 0 };
    Some(days_from_civil(y, m, d) * 86400 + hh * 3600 + mm * 60 - off)
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn time_now() -> String {
    // 不引第三方 crate：用系統時鐘 + UTC 偏移粗略格式化
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86400;
    let rem = secs % 86400;
    let (h, m) = ((rem / 3600 + 8) % 24, (rem % 3600) / 60);
    // 由 1970-01-01 起算民用歷
    let (y, mo, d) = civil_from_days(days as i64);
    format!("{:04}-{:02}-{:02} {:02}:{:02} UTC+8", y, mo, d, h, m)
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}