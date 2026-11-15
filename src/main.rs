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

// 顯示開關：首發狀態與信心星不帶量化資訊，暫時隱藏；代碼保留，日後可一行開返。
const SHOW_CONF: bool = false;
const SHOW_LINEUP: bool = false;
const SHOW_QUAL: bool = false;   // 射門質素：回測無增益（0.5897 vs 0.5893），用戶決定收埋；一行開返
const SHOW_TAGLINE: bool = false; // 頁頂 tagline「香港賽馬會賽程 × FotMob…」：用戶要求收埋（m03164）
const SHOW_STATS: bool = false;   // 頁頂統計五格（未開賽／比賽中／已完場／已出預測／聯賽）：用戶要求收埋（m03164）
const AH_NOTE: bool = false;      // 亞盤表註腳（走水＝和、分佈族差異說明）：用戶要求收埋（m03241）
const SHOW_META: bool = false;    // 頁底三塊（流程合規／Step 5 校準回測／聯賽接駁）：用戶要求收埋（m05921）
const SHOW_FOOT_META: bool = false; // 頁腳 資料時間／範圍／資料來源：用戶要求只留免責（m05921）

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

fn fmt_line(x: f64) -> String {
    if (x - x.round()).abs() < 1e-9 {
        format!("{:.0}", x)
    } else {
        let s = format!("{:.2}", x);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn qual_block(p: &Value) -> String {
    if !SHOW_QUAL { return String::new(); }
    let q = match get(p, &["diag", "quality"]) { Some(v) => v, None => return String::new() };
    let side = |k: &str| get(q, &[k]);
    let (h, a) = match (side("home"), side("away")) { (Some(h), Some(a)) => (h, a), _ => return String::new() };
    let f1 = |v: &Value, k: &str, d: usize| format!("{:.*}", d, num(v, &[k], f64::NAN));
    let n = num(h, &["n"], 0.0);
    format!(
        "<div class=\"ahd ql\"><div class=\"ahd-head\"><b>射門質素</b><span>近 {n:.0} 場 · xG＝量×質</span></div><table class=\"ahd-t\"><tr><th>項目</th><th>主</th><th>客</th></tr><tr><td class=\"ln\">xG 攻/守</td><td>{}</td><td>—</td></tr><tr><td class=\"ln\">射門 攻/守</td><td>{}</td><td>—</td></tr><tr><td class=\"ln\">xG/射門 攻/守</td><td>{}</td><td>—</td></tr><tr><td class=\"ln\">禁區內射門</td><td>{}</td><td>{}</td></tr><tr><td class=\"ln\">禁區觸球</td><td>{}</td><td>{}</td></tr><tr><td class=\"ln\">大機會</td><td>{}</td><td>{}</td></tr></table><div class=\"ahd-note\">只作解釋，不參與 λ：回測（5 大×3 季×4 摺，n=3,642）拆解版 Brier 0.5897 對純 xG 0.5893，無增益，故保留 xG 直接擬合。</div></div>",
        format!("{} / {}", f1(h, "xgF", 2), f1(h, "xgA", 2)),
        format!("{} / {}", f1(h, "shF", 1), f1(h, "shA", 1)),
        format!("{} / {}", f1(h, "qF", 3), f1(h, "qA", 3)),
        f1(h, "sibF", 1), f1(a, "sibF", 1),
        f1(h, "tobF", 1), f1(a, "tobF", 1),
        f1(h, "bcF", 2), f1(a, "bcF", 2),
    )
}

fn ahd_block(p: &Value) -> String {
    let mark = num(p, &["ahd", "mark"], 0.0);
    let rows = match get(p, &["ahd", "rows"]).and_then(|x| x.as_arr()) {
        Some(a) if !a.is_empty() => a,
        _ => return String::new(),
    };
    let dir = if mark > 0.0 {
        format!("客讓 {}（主 +{}）", fmt_line(mark), fmt_line(mark))
    } else if mark < 0.0 {
        format!("主讓 {}", fmt_line(-mark))
    } else {
        String::from("平手盤")
    };
    let mut body = String::new();
    for r in rows {
        let line = num(r, &["line"], 0.0);
        if (line - mark).abs() > 0.5001 {
            continue;
        }
        let is_q = ((line * 4.0).round() as i64).abs() % 2 == 1;
        let cell = |k: &str| format!("<td>{:.1}</td>", num(r, &[k], 0.0) * 100.0);
        let dash = String::from("<td class=\"dim\">—</td>");
        let lab = if line > 0.0 { format!("+{}", fmt_line(line)) } else { fmt_line(line) };
        let fav = get(r, &["fav"]).and_then(|x| x.as_str()).unwrap_or("h");
        let favtxt = if fav == "a" { "客" } else { "主" };
        let cls = if (line - mark).abs() < 1e-9 { " class=\"main\"" } else { "" };
        body.push_str(&format!(
            "<tr{class_}><td class=\"ln\">{lab}</td><td class=\"fav\">{favtxt}</td>{w}{hw}{pu}{hl}{lo}<td class=\"cv\">{cv:.1}</td></tr>",
            class_ = cls,
            lab = lab,
            favtxt = favtxt,
            w = cell("win"),
            hw = if is_q { cell("halfwin") } else { dash.clone() },
            pu = if is_q { dash.clone() } else { cell("push") },
            hl = if is_q { cell("halflose") } else { dash.clone() },
            lo = cell("lose"),
            cv = num(r, &["cover"], 0.0) * 100.0,
        ));
    }
    let note_html = if AH_NOTE {
        String::from("<div class=\"ahd-note\">全部由讓球方（上盤）視角；cover＝全贏＋¾×半贏＋½×走水＋¼×半輸（四分一盤＝兩注各半，故半贏算 ¾）。走水＝相對於該盤口的「和」——0 線走水就係 1X2 的和局，−1 線走水就係「剛好贏一球」。整數盤才有走水，四分一盤只有半贏／半輸，「—」＝結構上不可能。<br>本表（泊松＋DC）與上方「讓球 −1／+1」（負二項 ν=1.4，skill 指定）同盤口會有 1–4pp 差異，屬分佈族差異：回測顯示負二項對「讓球主客和」較準（Brier 0.8941 對 0.9266），泊松對亞盤 cover 較準（偏差 2.30pp 對 3.34pp），故此兩處刻意唔統一。</div>")
    } else {
        String::new()
    };
    format!(
        "<div class=\"ahd\"><div class=\"ahd-head\"><b>亞盤 AHD</b><span>{dir}</span></div><table class=\"ahd-t\"><tr><th>盤口</th><th>讓球方</th><th>全贏</th><th>半贏</th><th>走水</th><th>半輸</th><th>全輸</th><th>cover</th></tr>{body}</table>{note_html}</div>",
        dir = dir,
        body = body,
        note_html = note_html
    )
}

/// 覆盤 memo 區塊（hashtag 標籤 + 備註）。
/// 標籤由 build 時寫入 HTML（來源 fb/fixtures.mjs 讀 memos 表），唔靠前端 JS——
/// 用戶 m18618 見到全站標籤消失，根因就係「只靠 runtime fetch、API 一失敗就全空」。
/// 前端 JS（memoRender）之後仍然可以覆寫，屬 fail-open。
fn memo_block(id: &str, tags: &[String], note: &str) -> String {
    let has = !tags.is_empty();
    let chips: String = tags
        .iter()
        .map(|t| format!("<span class=\"mt\">#{}</span>", esc(t)))
        .collect();
    let sum = if has {
        let mut s = tags.join(" ");
        if !note.is_empty() {
            s.push_str(" · ");
            s.push_str(&note.chars().take(40).collect::<String>());
        }
        s
    } else {
        String::from("未寫")
    };
    let input_val = tags
        .iter()
        .map(|t| format!("#{}", t))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        r##"<div class="memo{cls}" data-mid="{id}">
  <div class="memo-head"><span class="memo-dot"></span><b>Memo</b><span class="memo-sum">{sum}</span></div>
  <div class="memo-tags">{chips}</div>
  <div class="memo-stat"></div>
  <div class="memo-body">
    <input class="memo-input" value="{inp}" placeholder="#初盤合理 終盤升水（寫法：#開頭，空格分隔形容詞；整句會當成一個標籤）" autocomplete="off">
    <textarea class="memo-note" placeholder="備註（例如：初盤 −0.5 合理，終盤退到 −0.25）">{note}</textarea>
    <div class="memo-acts"><button class="memo-save">儲存</button><button class="memo-clear ghost">清空</button><span class="memo-msg"></span></div>
    <div class="memo-hint">用 # 開頭加標籤（可多個，空格分隔）；標籤會正規化，例如「退盤」→「終盤退盤」。</div>
  </div>
</div>"##,
        cls = if has { " has" } else { "" },
        id = esc(id),
        sum = esc(&sum),
        chips = chips,
        inp = esc(&input_val),
        note = esc(note)
    )
}

fn card(f: &Value) -> String {    let comp = text(f, &["comp"]);
    let comp_name = text(f, &["compName"]);
    // 徽章顯示中文名（例如「墨西哥甲組聯賽」）；冇中文名先用馬會代碼（MD1）頂上。
    // 過濾仍然用馬會代碼（data-comp），唔受顯示改動影響。
    let comp_label = if comp_name.is_empty() { comp.clone() } else { comp_name.clone() };
    let home = text(f, &["home"]);
    let away = text(f, &["away"]);
    let kick = text(f, &["kick"]);
    let (md, hm) = kick_parts(&kick);
    let wd = weekday(&md);
    let status = text(f, &["status"]);
    let minute = text(f, &["minute"]);
    let sh = num(f, &["sh"], -1.0);
    let sa = num(f, &["sa"], -1.0);

    let p = f.get("pred").cloned().unwrap_or(Value::Null);
    // 早鳥預測（用戶要求卡片唔好吉住）：標明開賽前 60 分鐘會更新
    let is_early = text(&p, &["pass"]) == "early";
    let early_html = if is_early { String::from("<div class=\"early\">早鳥預測（開賽前 60 分鐘會更新）</div>") } else { String::new() };
    let has_pred = p.get("base").and_then(|b| b.get("h")).is_some();
    // FotMob 有冇收錄呢場：冇 → 唔應該扮「等緊生成」，要老實講源頭無數據
    let has_fm = f.get("has_fm").and_then(|v| v.as_bool()).unwrap_or(true);
    // 原因由 fixtures.mjs 算好（冇數據源／無歷史／真係等生成），呢度只負責顯示
    let row_note = text(f, &["note"]);
    let ghost_note: String = if !row_note.is_empty() {
        row_note
    } else if has_fm {
        String::from("預測待生成（開賽前 60 分鐘會自動計算）")
    } else {
        String::from("FotMob 未收錄此場（無數據來源，暫時唔會出預測）")
    };
    let plain = esc(&format!("{} {} {} {}", comp, home, away, comp_name));
    // 監視器標記（fb/sanity.mjs 每日檢查，經 audit.json → fixtures.mjs 帶上嚟）
    let warn_items: Vec<String> = f
        .get("warn")
        .and_then(|v| v.as_arr())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(esc)).collect())
        .unwrap_or_default();
    let warn_html = if warn_items.is_empty() {
        String::new()
    } else {
        format!("<div class=\"warnchip\">⚠ 監視：{}</div>", warn_items.join("；"))
    };
    let id = text(f, &["id"]);
    // 標籤由 build 寫入（fixtures.mjs 讀 memos 表）→ 就算 API 掛咗，卡上仍然見到標籤
    let memo_tags: Vec<String> = f
        .get("tags")
        .and_then(|v| v.as_arr())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    let memo_note = text(f, &["memoNote"]);
    let memo_html = memo_block(&id, &memo_tags, &memo_note);
    let score_html = if (status == "live" || status == "ft") && sh >= 0.0 && sa >= 0.0 {
        format!("<div class=\"score\"><b>{:.0}</b><i>–</i><b>{:.0}</b></div>", sh, sa)
    } else if status == "ft" {
        String::from("<div class=\"score miss\" title=\"未取到最終比分（該聯賽未在數據源登記）\"><b>—</b></div>")
    } else if status == "live" {
        // 已開賽但數據源仲未有比分（FotMob 滯後）：顯示破折號，比分到咗由 60 秒輪詢補上
        String::from("<div class=\"vs\">–</div>")
    } else {
        String::from("<div class=\"vs\">vs</div>")
    };
    let pulse = if status == "live" {
        format!("<span class=\"pill live\"><em></em>{}</span>", esc(&if minute.is_empty() { "比賽中".into() } else { minute.clone() }))
    } else if status == "ft" {
        String::from("<span class=\"pill ft\">完場</span>")
    } else if status == "post" {
        format!("<span class=\"pill ft\">{}</span>", esc(&if minute.is_empty() { String::from("延賽") } else { minute.clone() }))
    } else {
        String::from("<span class=\"pass\">未開賽</span>")
    };
    // 關注星標：清單只存喺瀏覽器 localStorage（無帳號、無後端）
    let star_html = format!(
        "<button class=\"star\" type=\"button\" data-watch=\"{}\" aria-pressed=\"false\" aria-label=\"關注呢場\" title=\"關注呢場\">☆</button>",
        esc(&id)
    );

    if !has_pred {
        return format!(
            r#"<article class="card ghost" data-id="{id}" data-comp="{comp}" data-text="{plain}" data-status="{status}">
  <div class="card-top">
    <span class="badge">{comp_label}</span>
    <span class="kick">{md} 週{wd} {hm}</span>
    {pulse}
    {star}
  </div>
  {warn_html}
  <div class="teams">
    <div class="team home"><span class="name">{home}</span></div>
    {score_html}
    <div class="team away"><span class="name">{away}</span></div>
  </div>
  <div class="ghost-note">{ghost_note}</div>
  {memo_html}
</article>"#,
            id = esc(&id),
            comp = esc(&comp),
            comp_label = esc(&comp_label),
            status = esc(&status),
            home = esc(&home),
            away = esc(&away),
            plain = plain,
            md = md,
            wd = wd,
            hm = hm,
            pulse = pulse,
            star = star_html.clone(),
            score_html = score_html,
            ghost_note = esc(&ghost_note),
            warn_html = warn_html,
            memo_html = memo_html,
        );
    }

    let bh = num(&p, &["base", "h"], 0.0) * 100.0;
    let bd = num(&p, &["base", "d"], 0.0) * 100.0;
    let ba = num(&p, &["base", "a"], 0.0) * 100.0;
    let fh = num(&p, &["fin", "h"], 0.0);
    let fd = num(&p, &["fin", "d"], 0.0);
    let fa = num(&p, &["fin", "a"], 0.0);

    let lh = num(&p, &["diag", "lambda_adj", "lh"], 0.0);
    let la = num(&p, &["diag", "lambda_adj", "la"], 0.0);

    let elo_h = num(&p, &["diag", "elo", "home"], 1500.0);
    let elo_a = num(&p, &["diag", "elo", "away"], 1500.0);
    let n = num(&p, &["diag", "n"], 0.0);
    let o25 = num(&p, &["diag", "o25"], 0.0);
    let o35 = num(&p, &["diag", "o35"], 0.0);
    let btts = num(&p, &["diag", "btts"], 0.0);
    let conf = text(&p, &["fin", "conf"]);
    let cs = num(&p, &["fin", "cs"], 3.0);
    let lineup = text(&p, &["diag", "lineupState"]);
    let inj_h = arr_len(&p, &["diag", "injuries", "home"]);
    let inj_a = arr_len(&p, &["diag", "injuries", "away"]);
    let pass = text(&p, &["pass"]);
    let _pass_zh = match pass.as_str() { "t60" => "T-60", "t25" => "T-25", _ => "預覽" };

    let hc = |line: &str, k: &str| num(&p, &["diag", "handicap", line, k], 0.0);
    let (m1h, m1d, m1a) = (hc("-1", "h"), hc("-1", "d"), hc("-1", "a"));
    let (p1h, p1d, p1a) = (hc("1", "h"), hc("1", "d"), hc("1", "a"));

    let tops: Vec<String> = get(&p, &["diag", "tops"])
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
    let cal_html = {
        let ch = num(&p, &["cal", "h"], -1.0);
        if ch < 0.0 {
            // 無同分布校準器 → 明示原因（SKILL L134：唔可以硬套其他聯賽校準器）
            let err = text(&p, &["cal", "error"]);
            if err.is_empty() {
                String::new()
            } else {
                format!("<div class=\"calrow\" title=\"Step 5 同分布校準器不可用，本場只顯示基線\">校準：未提供（{}）</div>", esc(&err))
            }
        } else {
            let cd = num(&p, &["cal", "d"], 0.0);
            let ca = num(&p, &["cal", "a"], 0.0);
            let cn = num(&p, &["cal", "n"], 0.0);
            let cacc = num(&p, &["cal", "own", "acc"], 0.0);
            let cb = num(&p, &["cal", "own", "brier"], 0.0);
            let bb = num(&p, &["cal", "base", "brier"], 0.0);
            format!(
                "<div class=\"calrow\" title=\"Step 5 walk-forward logistic 校準；skill 第 227 行：cal 為參考層，不覆蓋最終值\">校準 <b>{:.1}%</b> <b>{:.1}%</b> <b>{:.1}%</b><i>n={:.0} · 命中 {:.1}% · Brier {:.3}（基線 {:.3}）</i></div>",
                ch * 100.0, cd * 100.0, ca * 100.0, cn, cacc * 100.0, cb, bb
            )
        }
    };
    let lineup_cls = if lineup.starts_with('✅') { "ok" } else if lineup.starts_with('◐') { "mid" } else { "low" };
    let ahd_html = ahd_block(&p);
    // 讓球盤（多莊共識）賠率區塊：按用戶要求唔喺頁面顯示（數據仍留喺 fixtures/pred 供後用）。
    let mktah_html = String::new();
    let qual_html = qual_block(&p);
    // 風險聲明（SKILL L236：fin.risks[] 必填）—— 數據仍寫入 pred，但頁面唔顯示（用戶要求還原原本樣貌）
    let risks_html = String::new();

    format!(
        r#"<article class="card" data-id="{id}" data-comp="{comp}" data-text="{plain}" data-status="{status}">
  <div class="card-top">
    <span class="badge">{comp_label}</span>
    <span class="kick">{md} 週{wd} {hm}</span>
    {pulse}
    {star}
  </div>
  {warn_html}
  <div class="teams">
    <div class="team home"><span class="name">{home}</span><span class="elo">Elo {elo_h:.0}</span></div>
    {score_html}
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
  <div class="fair">公平賠率（1X2）　主 {odds_h}　和 {odds_d}　客 {odds_a}</div>
  {early_html}
  {cal_html}
  <div class="meta">
    <span>λ {lh:.2} / {la:.2}</span>
    <span>大 2.5 {o25_p}</span>
    <span>大 3.5 {o35_p}</span>
    <span>BTTS {btts_p}</span>
    <span>樣本 {n:.0}</span>
  </div>
  <div class="hc">
    <div class="hc-row"><span class="hc-key">讓球 −1</span><span>{m1h_p}</span><span>{m1d_p}</span><span>{m1a_p}</span></div>
    <div class="hc-row"><span class="hc-key">讓球 +1</span><span>{p1h_p}</span><span>{p1d_p}</span><span>{p1a_p}</span></div>
  </div>
  {ahd_html}
  {mktah_html}
  {qual_html}
  {risks_html}
  {memo_html}
  <div class="tops">{tops}</div>
  <div class="foot">
    {conf_html}{lineup_html}
    {inj_chip}
  </div>
</article>"#,
        comp = esc(&comp),
        comp_label = esc(&comp_label),
        home = esc(&home),
        away = esc(&away),
        plain = plain,
        status = esc(&status),
        md = md,
        wd = wd,
        hm = hm,
        pulse = pulse,
        score_html = score_html,
        elo_h = elo_h,
        elo_a = elo_a,
        bh_p = format!("{:.1}", bh),
        bd_p = format!("{:.1}", bd),
        ba_p = format!("{:.1}", ba),
        fh_p = pct(fh),
        fd_p = pct(fd),
        fa_p = pct(fa),
        odds_h = if fh > 0.0 { format!("{:.2}", 1.0 / fh) } else { "—".into() },
        early_html = early_html,
        warn_html = warn_html,
        odds_d = if fd > 0.0 { format!("{:.2}", 1.0 / fd) } else { "—".into() },
        odds_a = if fa > 0.0 { format!("{:.2}", 1.0 / fa) } else { "—".into() },
        lh = lh,
        la = la,
        n = n,
        o25_p = pct(o25),
        o35_p = pct(o35),
        btts_p = pct(btts),
        m1h_p = pct(m1h),
        m1d_p = pct(m1d),
        m1a_p = pct(m1a),
        p1h_p = pct(p1h),
        p1d_p = pct(p1d),
        p1a_p = pct(p1a),
        tops = tops.join(""),
        conf_html = if SHOW_CONF { format!("<span class=\"conf\" data-cs=\"{:.0}\">{}</span>", cs, esc(&conf)) } else { String::new() },
        lineup_html = if SHOW_LINEUP { format!("<span class=\"tag {}\">首發 {}</span>", lineup_cls, esc(&lineup)) } else { String::new() },
        inj_chip = inj_chip,
        star = star_html,
        ahd_html = ahd_html,
        mktah_html = mktah_html,
        qual_html = qual_html,
        risks_html = risks_html,
        memo_html = memo_html,
    )
}

// ---------- 頁面 ----------

const CSS: &str = r#"
.calrow{margin:2px 0 8px;padding:6px 10px;border-radius:10px;background:rgba(76,155,161,.10);border:1px dashed rgba(76,155,161,.38);font-size:12px;color:#2b616a;display:flex;gap:8px;align-items:center;flex-wrap:wrap}
.calrow b{font-variant-numeric:tabular-nums;color:#2b616a}
.calrow i{margin-left:auto;font-style:normal;color:#5c8b8f;font-size:11px}
.mktah{margin:6px 0 0;border-top:1px dashed rgba(76,155,161,.28);padding-top:6px}
.mktah .ahd-head b{color:#2b616a;font-weight:600}
.ahd{margin:6px 0 0;border-top:1px dashed rgba(76,155,161,.28);padding-top:6px}
.ahd-head{display:flex;gap:8px;align-items:baseline;font-size:11px;color:#5c8b8f;margin-bottom:3px}
.ahd-head b{color:#2b616a;font-weight:600}
.ahd-t{width:100%;border-collapse:collapse;font-size:11px;font-variant-numeric:tabular-nums}
.ahd-t th{font-weight:500;color:#7d9fa2;text-align:right;padding:1px 3px}
.ahd-t td{text-align:right;padding:1px 3px;color:#3f6f74}
.ahd.ql{margin-top:4px}
.score.miss b{color:#9aa8ab;font-weight:600}
.ahd-t td.fav{color:var(--cyan);font-weight:600}
.ahd-t td.ln{text-align:left;color:#2b616a;font-weight:600}
.ahd-t td.dim{color:#c3d6d8}
.ahd-t td.cv{color:#4c9ba1;font-weight:600}
.ahd-t tr.main td{background:rgba(76,155,161,.10)}
.ahd-t tr.main td.ln{color:#4c9ba1}
.ahd-note{margin-top:3px;font-size:10px;color:#9dbfc1}
.cal-table td,.cal-table th{font-variant-numeric:tabular-nums;font-size:12.5px}
.cal-table .up{color:#2e7d5b;font-weight:600}.cal-table .down{color:#b0553f;font-weight:600}

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
.badge{background:var(--cyan);color:#fff;border-radius:7px;padding:2px 8px;font-size:11px;letter-spacing:.02em;font-weight:600;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;max-width:62%}
.kick{margin-left:auto;font-variant-numeric:tabular-nums;color:var(--ink-2)}
.pass{border:1px solid var(--cyan-3);color:var(--cyan);border-radius:6px;padding:1px 6px;font-size:11px}

.teams{display:flex;align-items:center;gap:10px;margin:16px 0 14px;position:relative}
.team{flex:1;display:flex;flex-direction:column;gap:3px;min-width:0}
.team.away{align-items:flex-end;text-align:right}
.team .name{font-size:17.5px;font-weight:650;letter-spacing:.01em;display:flex;align-items:center;min-width:0;max-width:100%;overflow:hidden}
.team .name .nm{min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.crest{width:20px;height:20px;object-fit:contain;vertical-align:-4px;flex:0 0 auto}
.crest-l{margin-right:5px}
.crest-r{margin-left:5px}
.wteam .crest{width:17px;height:17px;vertical-align:-3px}
.wteam .crest-l{margin-right:4px}
.wteam .crest-r{margin-left:4px}
.team .elo{font-size:11.5px;color:var(--muted);font-variant-numeric:tabular-nums}
.vs{color:var(--muted);font-size:12px;letter-spacing:.1em}

.bar{display:flex;height:9px;border-radius:999px;overflow:hidden;background:var(--mist-2);position:relative}
.bar .seg{display:block;height:100%}
.bar .h{background:var(--home)} .bar .d{background:var(--draw)} .bar .a{background:var(--away)}

.nums{display:grid;grid-template-columns:repeat(3,1fr);margin-top:10px;text-align:center}
.nums .n b{display:block;font-size:19px;font-variant-numeric:tabular-nums;letter-spacing:.01em}
.nums .n i{display:block;font-style:normal;font-size:11.5px;color:var(--muted);margin-top:1px}
.nums .h b{color:var(--home)} .nums .d b{color:#7f9a9c} .nums .a b{color:var(--away)}
.early{margin-top:6px;font-size:11px;color:#f0b429;text-align:center;letter-spacing:.04em}
.warnchip{margin-top:6px;font-size:11px;line-height:1.45;color:#ffb4a2;background:rgba(255,120,90,.10);border:1px solid rgba(255,120,90,.28);border-radius:8px;padding:5px 8px}
.fair{margin-top:8px;padding:6px 10px;border:1px dashed var(--line);border-radius:8px;font-size:11.5px;color:var(--muted);text-align:center;font-variant-numeric:tabular-nums;letter-spacing:.02em}

.meta{display:flex;flex-wrap:wrap;gap:6px 12px;margin-top:12px;font-size:12px;color:var(--ink-2);
  font-variant-numeric:tabular-nums}
.meta span{position:relative}

.hc{margin-top:12px;border-top:1px dashed var(--line);padding-top:9px;font-size:12.5px;
  font-variant-numeric:tabular-nums;color:var(--ink-2)}
.hc-row{display:grid;grid-template-columns:78px 1fr 1fr 1fr;gap:4px;padding:2px 0}
.hc-row span{text-align:right}
.hc-key{text-align:left !important;color:var(--muted)}

.tops{display:flex;flex-wrap:wrap;gap:6px;margin-top:11px}
.risks{margin:9px 0 0;padding:8px 10px 8px 26px;border-radius:10px;background:rgba(229,180,84,.08);border:1px solid rgba(229,180,84,.28);font-size:11.8px;color:#c9a558;line-height:1.6}
.risks li{margin:2px 0}
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
.bver{color:var(--muted);font-size:11px;letter-spacing:.02em}
.empty{padding:40px;text-align:center;color:var(--muted)}
@media (max-width:520px){
  .grid{grid-template-columns:1fr}
  .hero{padding-top:44px}
}

/* ── 分頁 / 日期分組 / 比分 ── */
.tabs{display:flex;gap:8px;margin:20px 0 6px;flex-wrap:wrap}
.tab{appearance:none;border:1px solid var(--line);background:rgba(255,255,255,.6);color:var(--ink-2);
  border-radius:999px;padding:8px 16px;font-size:14px;font-weight:600;cursor:pointer;display:flex;align-items:center;gap:7px;
  transition:background .18s,border-color .18s,color .18s}
.tab b{font-size:12px;font-weight:700;color:var(--muted);background:rgba(21,48,47,.06);border-radius:999px;padding:1px 7px}
.tab:hover{border-color:var(--cyan-3)}
.tab.on{background:var(--cyan);border-color:var(--cyan);color:#fff}
.tab.on b{color:#fff;background:rgba(255,255,255,.22)}
.tabbody.off{display:none}
.dayhead{display:flex;align-items:baseline;gap:10px;margin:20px 0 12px;padding-bottom:8px;
  border-bottom:1px dashed rgba(21,48,47,.14);scroll-margin-top:120px}
.daybar{position:sticky;top:56px;z-index:15;display:flex;gap:6px;flex-wrap:nowrap;
  overflow-x:auto;overflow-y:hidden;margin:14px 0 0;padding:6px 0;background:var(--bg);
  -webkit-overflow-scrolling:touch;scrollbar-width:none;-ms-overflow-style:none}
.daybar::-webkit-scrollbar{display:none}
.daynav{border:1px solid var(--line);background:rgba(255,255,255,.75);color:var(--ink-2);
  font:inherit;font-size:12px;border-radius:999px;padding:3px 10px;cursor:pointer;
  flex:0 0 auto;white-space:nowrap}
.daynav em{font-style:normal;color:var(--muted);margin-left:4px;font-size:11px}
.daynav:hover{border-color:var(--cyan-2);color:var(--cyan-2)}
.daynav.on{background:var(--cyan);color:#fff;border-color:var(--cyan)}
.daynav.on em{color:rgba(255,255,255,.8)}
.monthsel{flex:0 0 auto;position:sticky;left:0;z-index:2;font:inherit;font-size:12px;
  border:1px solid var(--line);border-radius:999px;background:var(--bg);color:var(--ink-2);
  padding:3px 6px;margin-right:2px;cursor:pointer}
.daypage.off{display:none}
.dayhead b{font-size:15px;letter-spacing:.3px}
.dayhead span{font-size:12px;color:var(--muted)}
.score{display:flex;align-items:center;gap:5px;font-size:22px;font-weight:800;color:var(--ink);line-height:1}
.score i{font-style:normal;opacity:.35;font-weight:600}
.pill{display:inline-flex;align-items:center;gap:5px;border-radius:6px;padding:2px 7px;font-size:11px;font-weight:700;letter-spacing:.2px}
.pill.live{background:#e6f6ee;color:#1e7a50;border:1px solid #bfe6d2}
.pill.live em{width:6px;height:6px;border-radius:50%;background:#22a06b;display:inline-block;animation:pulse 1.6s infinite}
@keyframes pulse{0%,100%{opacity:1}50%{opacity:.25}}
.pill.ft{background:rgba(21,48,47,.05);color:var(--ink-2);border:1px solid var(--line)}
.pill.soon{background:rgba(76,155,161,.08);color:var(--cyan-2);border:1px solid rgba(76,155,161,.28)}
.card.ghost{background:rgba(255,255,255,.45);border-style:dashed}
.card.ghost .ghost-note{margin-top:12px;font-size:12px;color:var(--muted)}
.card.ghost .team .name{font-weight:600}

/* ── 覆盤 memo（hashtag 標籤） ── */
.memo{margin-top:12px;border-top:1px dashed rgba(21,48,47,.16);padding-top:10px}
.memo-head{display:flex;align-items:center;gap:8px;font-size:12px;color:var(--muted);cursor:pointer;user-select:none}
.memo-head b{font-size:12px;font-weight:700;color:var(--ink-2)}
.memo-head .memo-dot{width:7px;height:7px;border-radius:50%;background:transparent;border:1px solid var(--line)}
.memo.has .memo-dot{background:var(--cyan);border-color:var(--cyan)}
.memo-tags{display:flex;flex-wrap:wrap;gap:5px;margin-top:8px}
.memo-stat{margin-top:6px}
.memo-stat-line{font-size:10.5px;color:var(--muted);line-height:1.7}
.memo-stat-line b{color:var(--cyan-2);font-weight:700}
.memo-tags .mt{font-size:11px;font-weight:700;color:var(--cyan-2);background:rgba(76,155,161,.12);
  border:1px solid rgba(76,155,161,.35);border-radius:999px;padding:2px 9px}
.memo-body{margin-top:8px;display:none}
.memo.open .memo-body{display:block}
.memo-body input,.memo-body textarea{width:100%;box-sizing:border-box;font:inherit;font-size:12px;
  border:1px solid var(--line);border-radius:8px;padding:6px 9px;background:rgba(255,255,255,.75);color:var(--ink);margin-bottom:6px}
.memo-body textarea{min-height:52px;resize:vertical}
.memo-acts{display:flex;align-items:center;gap:8px}
.memo-acts button{font:inherit;font-size:12px;font-weight:700;border-radius:8px;padding:5px 12px;cursor:pointer;
  border:1px solid var(--cyan);background:var(--cyan);color:#fff}
.memo-acts button.ghost{background:transparent;color:var(--ink-2);border-color:var(--line)}
.memo-acts .memo-msg{font-size:11px;color:var(--muted)}
.memo-hint{font-size:11px;color:var(--muted);margin-top:4px}
/* ── 關注（本機清單，無帳號） ── */
.star{margin-left:auto;appearance:none;background:transparent;border:1px solid var(--line);color:var(--muted);
  border-radius:999px;font:inherit;font-size:12px;line-height:1;padding:3px 8px;cursor:pointer;flex:0 0 auto}
.star:hover{border-color:var(--cyan-2);color:var(--cyan)}
.star.on{color:#9a6b00;border-color:rgba(184,134,11,.45);background:rgba(240,198,116,.18)}
.watchbtn{appearance:none;border:1px solid var(--line);background:rgba(255,255,255,.6);color:var(--ink-2);
  border-radius:12px;padding:9px 14px;font:inherit;font-size:13px;font-weight:700;cursor:pointer;
  display:flex;gap:6px;align-items:center;flex:0 0 auto}
.watchbtn:hover{border-color:var(--cyan-2)}
.watchbtn b{background:var(--cyan);color:#fff;border-radius:999px;padding:1px 7px;font-size:11px;font-weight:700}
.wmodal{position:fixed;inset:0;z-index:80;display:none;background:rgba(10,26,28,.55);
  -webkit-backdrop-filter:blur(3px);backdrop-filter:blur(3px);padding:20px 12px 40px;overflow:auto}
.wmodal.show{display:block}
.wnav{max-width:880px;margin:0 auto;background:var(--mist-0);border:1px solid var(--line);border-radius:18px;
  box-shadow:0 18px 50px rgba(10,26,28,.28);padding:16px 16px 18px}
.wnav-head{display:flex;align-items:center;gap:10px;font-size:15px;color:var(--ink);
  border-bottom:1px solid var(--line);padding-bottom:10px}
.wnav-head .wsum{font-size:12px;color:var(--muted);font-weight:400}
.wnav-head .wclose{margin-left:auto;appearance:none;border:1px solid var(--line);background:transparent;
  color:var(--ink-2);border-radius:10px;width:30px;height:30px;cursor:pointer;font-size:14px;line-height:1}
.wlist{margin-top:12px;display:flex;flex-direction:column;gap:10px}
/* 關注頁每一場＝一張卡，四行由上至下：時間／賽事 → 隊名＋比數 → 標籤 → 同類統計 */
.wcard{background:#fff;border:1px solid var(--line);border-radius:14px;padding:10px 12px 11px;cursor:pointer;
  transition:border-color .15s ease,box-shadow .15s ease}
.wcard:hover{border-color:var(--cyan-2);box-shadow:0 3px 14px rgba(10,26,28,.07)}
.wcard:focus-visible{outline:2px solid var(--cyan-2);outline-offset:2px}
.wcard-top{display:flex;align-items:center;gap:8px;font-size:11px;color:var(--muted);line-height:1.4}
.wcard-top .wt{white-space:nowrap;font-variant-numeric:tabular-nums}
.wcard-top .wcomp{min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;
  color:var(--muted);border-left:1px solid var(--line);padding-left:8px}
.wcard-top .pill{margin-left:auto;flex:0 0 auto}
.wcard-top .wdel{flex:0 0 auto;border:0;background:transparent;color:var(--muted);font:inherit;font-size:13px;
  line-height:1;padding:3px 5px;border-radius:8px;cursor:pointer}
.wcard-top .wdel:hover{background:rgba(206,74,68,.12);color:#c0392b}
.wcard-mid{display:grid;grid-template-columns:1fr auto 1fr;align-items:baseline;gap:6px 10px;margin-top:8px}
.wteam{font-size:13.5px;font-weight:700;color:var(--ink);line-height:1.35;
  display:flex;align-items:center;min-width:0;max-width:100%;overflow:hidden}
.wteam .nm{min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.wteam.away{text-align:right;justify-content:flex-end}
.wsc{font-size:14px;font-weight:800;color:var(--cyan);white-space:nowrap;font-variant-numeric:tabular-nums}
.wsc i{font-style:normal;font-weight:400;font-size:11px;color:var(--muted)}
.wcard-tags{display:flex;flex-wrap:wrap;gap:5px;margin-top:9px}
.wcard-tags span{font-size:10.5px;line-height:1.6;color:var(--cyan-2);background:rgba(76,155,161,.10);
  border:1px solid rgba(76,155,161,.3);border-radius:999px;padding:1px 8px;white-space:nowrap}
.wcard-tags .pig{color:#9a6b00;border-color:rgba(184,134,11,.4);background:rgba(240,198,116,.16)}
.wcard-stat{margin-top:7px;padding-top:7px;border-top:1px dashed rgba(21,48,47,.12);
  font-size:10.5px;color:var(--muted);line-height:1.55;letter-spacing:.2px}
.wcard-stat b{color:var(--ink-2)}
.wempty{font-size:13px;color:var(--muted);padding:18px 4px;line-height:1.9}
.wnotify{display:flex;flex-wrap:wrap;align-items:center;gap:8px;margin-top:10px;padding-top:10px;border-top:1px dashed rgba(21,48,47,.16)}
.wpushbtn{font:inherit;font-size:12px;font-weight:600;padding:7px 12px;border-radius:999px;border:1px solid var(--line);background:rgba(255,255,255,.66);color:var(--ink-2);cursor:pointer}
.wpushbtn:disabled{opacity:.55;cursor:not-allowed}
.wpushbtn.on{background:rgba(58,208,200,.16);border-color:var(--cyan);color:#0c3f3d}
.wpushmsg{font-size:11px;color:var(--muted);line-height:1.6;flex:1 1 180px}
"#;

const WATCH_HTML: &str = r#"
  <div id="wmodal" class="wmodal" role="dialog" aria-modal="true" aria-labelledby="wtitle">
    <div class="wnav">
      <div class="wnav-head">
        <b id="wtitle">我關注嘅賽事</b>
        <span id="wsum" class="wsum"></span>
        <button id="wclose" class="wclose" type="button" aria-label="關閉">✕</button>
      </div>
      <div id="wbody" class="wlist"></div>
      <div class="wnotify">
        <button id="wpush" class="wpushbtn" type="button">🔔 開啟通知</button>
        <span id="wpushmsg" class="wpushmsg"></span>
      </div>
      <div class="wempty" style="padding:10px 4px 0;font-size:11px">清單只存喺你呢部機嘅瀏覽器（localStorage），冇帳號、唔會上傳。</div>
    </div>
  </div>
"#;

const JS: &str = r#"
(function(){
  const cards=[...document.querySelectorAll('.card')];
  // ── 覆盤 memo（hashtag 標籤）：runtime 經 fbapi /memos 讀寫 ─────────────
  const MEMO_API='https://oddsapi.2828666.xyz/fbW8FCX0tNblXanTXGyTUB/memos';
  const memoBoxes=[...document.querySelectorAll('.memo')];
  const memoCache={};
  const MEMO_ALL={};  // 全部 memo（唔止頁面上有卡嘅），關注頁要用
  const memoStats={tag:{},dim:{}};
  // 完整標籤的歷史勝率（已結算場次）：一行、只顯示勝率 + 場數
  function memoStatLine(tags){
    const s=memoStats.tag[tags];
    if(!s||!s.n) return '';
    const nn=(s.cl_n!=null?s.cl_n:s.n);
    if(!nn||s.up_hit==null) return '';
    const up=Math.round(s.up_hit*100), dn=(s.down_hit==null?'—':Math.round(s.down_hit*100)+'%');
    // 走水（終盤讓球開和）既唔屬上盤亦唔屬下盤，唔寫出嚟就會出現「50%+25%=75%」嘅疑問（用戶 m01115）。
    const pu=(s.push_rate==null?null:Math.round(s.push_rate*100));
    const puTxt=(pu==null?'':' · 走水 <b>'+pu+'%</b>');
    return '<div class="memo-stat-line">同類標籤（'+nn+' 場）：上盤 <b>'+up+'%</b> · 下盤 <b>'+dn+'</b>'+puTxt+'</div>';
  }
  function memoRender(box, memo){
    const tags=box.querySelector('.memo-tags');
    const sum=box.querySelector('.memo-sum');
    const inp=box.querySelector('.memo-input');
    const note=box.querySelector('.memo-note');
    const stat=box.querySelector('.memo-stat');
    if(memo){
      box.classList.add('has');
      sum.textContent=(memo.tags&&memo.tags.length?memo.tags.join(' '):'')+(memo.note?((memo.tags&&memo.tags.length)?' · ':'')+memo.note.slice(0,40):'')||'已寫';
      tags.innerHTML=(memo.tags||[]).map(t=>'<span class="mt">#'+t.replace(/[<>&]/g,'')+'</span>').join('');
      if(stat) stat.innerHTML=memoStatLine((memo.tags||[]).join('·'));
      if(document.activeElement!==inp) inp.value=(memo.tags||[]).map(t=>'#'+t).join(' ');
      if(document.activeElement!==note) note.value=memo.note||'';
    } else {
      box.classList.remove('has');
      sum.textContent='未寫';
      tags.innerHTML='';
      if(stat) stat.innerHTML='';
    }
  }
  async function memoLoad(){
    try{
      // 標籤歷史勝率（已結算樣本）
      try{
        const sr=await fetch(MEMO_API+'/stats',{cache:'no-store'});
        if(sr.ok){
          const sj=await sr.json();
          (sj.stats||[]).forEach(s=>{memoStats.tag[s.tag]=s;});
          (sj.dims||[]).forEach(d=>{memoStats.dim[d.tag]=d;});
        }
      }catch(e){/* 靜靜失敗 */}
      const r=await fetch(MEMO_API+'?all=1',{cache:'no-store'});
      if(!r.ok) return;
      const j=await r.json();
      const map={};
      (j.memos||[]).forEach(m=>{map[m.mid]=m;MEMO_ALL[m.mid]=m;});
      memoBoxes.forEach(box=>{
        const mid=box.dataset.mid;
        if(map[mid]){memoCache[mid]=map[mid];memoRender(box,map[mid]);}
      });
    }catch(e){/* 靜靜失敗 */ }
  }
  memoBoxes.forEach(box=>{
    const head=box.querySelector('.memo-head');
    head.addEventListener('click',()=>{box.classList.toggle('open');});
    const msg=box.querySelector('.memo-msg');
    box.querySelector('.memo-save').addEventListener('click',async()=>{
      const mid=box.dataset.mid;
      const raw=box.querySelector('.memo-input').value;
      const tags=raw.split(/[\s,，、]+/).filter(Boolean);
      const note=box.querySelector('.memo-note').value;
      msg.textContent='儲存中…';
      try{
        const r=await fetch(MEMO_API,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({mid,tags,note})});
        const j=await r.json();
        if(j.ok){memoCache[mid]=j.memo;memoRender(box,j.memo);msg.textContent='已儲存';setTimeout(()=>{msg.textContent='';},2500);}
        else msg.textContent='失敗：'+(j.error||r.status);
      }catch(e){msg.textContent='失敗（網絡）';}
    });
    box.querySelector('.memo-clear').addEventListener('click',async()=>{
      const mid=box.dataset.mid;
      msg.textContent='清空中…';
      try{
        const r=await fetch(MEMO_API,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({mid,tags:[],note:''})});
        const j=await r.json();
        if(j.ok){delete memoCache[mid];memoRender(box,null);msg.textContent='已清空';setTimeout(()=>{msg.textContent='';},2500);}
        else msg.textContent='失敗：'+(j.error||r.status);
      }catch(e){msg.textContent='失敗（網絡）';}
    });
  });
  memoLoad();
  const chips=[...document.querySelectorAll('.fchip')];
  const tabs=[...document.querySelectorAll('.tab')];
  const bodies=[...document.querySelectorAll('.tabbody')];
  const q=document.getElementById('q');
  let comp='all';
  function tidy(){
    for(const head of document.querySelectorAll('.dayhead')){
      const grid=head.nextElementSibling;
      if(!grid) continue;
      const any=[...grid.querySelectorAll('.card')].some(c=>c.style.display!=='none');
      head.style.display=any?'':'none';
      grid.style.display=any?'':'none';
    }
  }
  function apply(){
    const t=(q.value||'').trim().toLowerCase();
    const searching = (!!t) || comp!=='all';
    for(const c of cards){
      const okComp = comp==='all' || c.dataset.comp===comp;
      const okText = !t || (c.dataset.text||'').toLowerCase().includes(t);
      c.style.display = okComp&&okText?'':'none';
    }
    tidy();
    // 已完場分頁：搜尋時展開全部日子（跨日搵），清空搜尋就返返去現時日子
    if(dpages.length){
      const db=document.querySelector('.daybar');
      if(db) db.style.display = searching?'none':'';
      if(searching){
        dpages.forEach(p=>p.classList.remove('off'));
        dnav.forEach(x=>x.classList.remove('on'));
      } else {
        showDay(curDay || (dnav[0] && dnav[0].dataset.day));
      }
    }
  }
  function showTab(id){
    tabs.forEach(x=>x.classList.toggle('on', x.dataset.tab===id));
    bodies.forEach(x=>x.classList.toggle('off', x.dataset.body!==id));
  }
  tabs.forEach(t=>t.addEventListener('click',()=>showTab(t.dataset.tab)));
  // 已完場：日子分頁（一日一頁；搜尋時全部展開）
  const dnav=[...document.querySelectorAll('.daynav')];
  const dpages=[...document.querySelectorAll('.daypage')];
  let curDay=dnav.length?dnav[0].dataset.day:null;
  // 日子 bar 只有一行、橫向滑動：選中嘅日子自動捲到中間，唔會睇唔到
  function dayScroll(b){
    const bar=b&&b.parentElement;
    if(!bar||typeof bar.scrollTo!=="function"||typeof b.offsetLeft!=="number") return;
    const left=Math.max(0, b.offsetLeft-bar.clientWidth/2+(b.offsetWidth||0)/2);
    try{ bar.scrollTo({left:left,behavior:"smooth"}); }catch(e){ bar.scrollLeft=left; }
  }
  function showDay(id){
    curDay=id;
    dnav.forEach(x=>{
      const on=x.dataset.day===id;
      x.classList.toggle('on', on);
      if(on) dayScroll(x);
    });
    dpages.forEach(p=>p.classList.toggle('off', p.dataset.day!==id));
  }
  // 月份多過一個 → 日子 bar 左邊加「月份」選擇（chips 只顯示該月）：將來存到幾個月都唔會失控
  function initMonthSel(){
    const bar=document.querySelector('.daybar');
    if(!bar||!dnav.length) return null;
    const mon=[];
    dnav.forEach(x=>{const mm=String(x.dataset.day||'').replace('day-','').slice(0,2); if(mm&&mon.indexOf(mm)<0) mon.push(mm);});
    if(mon.length<2) return null;
    const sel=document.createElement('select');
    sel.className='monthsel';
    sel.setAttribute('aria-label','選月份');
    mon.forEach(mm=>{const o=document.createElement('option');o.value=mm;o.textContent=Number(mm)+'月';sel.appendChild(o);});
    function pick(mm){
      dnav.forEach(x=>{
        const m=String(x.dataset.day||'').replace('day-','').slice(0,2);
        x.style.display=(m===mm)?'':'none';
      });
      const first=dnav.filter(x=>x.style.display!=='none')[0];
      if(first) showDay(first.dataset.day);
      if(typeof bar.scrollTo==='function'){try{bar.scrollTo({left:0});}catch(e){bar.scrollLeft=0;}}
      else bar.scrollLeft=0;
    }
    sel.addEventListener('change',function(){pick(sel.value);});
    bar.insertBefore(sel, bar.firstChild);
    pick(mon[0]);
    return sel;
  }
  const MSEL=initMonthSel();
  globalThis.__mdms=initMonthSel;
  dnav.forEach(b=>b.addEventListener('click',()=>{
    if(q) q.value='';
    comp='all';
    chips.forEach(x=>x.classList.remove('on'));
    if(chips[0]) chips[0].classList.add('on');
    showDay(b.dataset.day);
    apply();
  }));
  chips.forEach(ch=>ch.addEventListener('click',()=>{
    chips.forEach(x=>x.classList.remove('on'));
    ch.classList.add('on'); comp=ch.dataset.filter; apply();
  }));
  if(q) q.addEventListener('input',apply);
  apply();
  // ── 靜靜自動更新：每 60s 拉 data.json，只改有變嘅卡 ──────────────────
  const snap={};
  cards.forEach(c=>{
    if(!c.dataset.id) return;
    const pill=c.querySelector('.pill,.pass');
    const sb=[...c.querySelectorAll('.score b')].map(x=>x.textContent);
    snap[c.dataset.id]={status:c.dataset.status, minute:pill?pill.textContent.replace('比賽中',''):'' , sh:sb[0], sa:sb[1]};
  });
  const ftBody=document.querySelector('[data-body="ft"]');
  const liveBody=document.querySelector('[data-body="live"]');
  const WD=['日','一','二','三','四','五','六'];
  function mdOf(kick){const s=String(kick||'');return s.length>10?s.slice(5,7)+'/'+s.slice(8,10):'';}
  function labelOf(kick,md){const y=Number(String(kick||'').slice(0,4))||new Date().getFullYear();const m=Number(md.slice(0,2)),d=Number(md.slice(3));return md+' 週'+WD[new Date(y,m-1,d).getDay()];}
  // 分頁係靜態 HTML：狀態變咗而唔搬卡，張卡就會一直留喺舊分頁（用戶 m17055 報嘅就係呢個）。
  function ensureGroup(host,md,kick){
    const id='day-'+md.replace('/','-');
    const head=host.querySelector('#'+id);
    if(head){const g=head.nextElementSibling;if(g&&g.classList.contains('grid')){return {head:head,grid:g};}}
    const tpl=document.createElement('div');
    tpl.innerHTML='<div class="dayhead" id="'+id+'"><b>'+labelOf(kick,md)+'</b><span>0 場</span></div><div class="grid"></div>';
    const nh=tpl.firstElementChild, ng=nh.nextElementSibling;
    let before=null;
    for(const h of host.querySelectorAll('.dayhead')){
      const hm=(h.id||'').replace('day-','').replace('-','/');
      if(hm>md){before=h;break;}
    }
    if(before){host.insertBefore(nh,before);host.insertBefore(ng,before);}
    else{host.appendChild(nh);host.appendChild(ng);}
    return {head:nh,grid:ng};
  }
  function placeCard(body,card,kick){
    if(!body||!card) return;
    body.querySelectorAll('.panel.empty').forEach(function(p){p.remove();});
    const md=mdOf(kick)||mdOf(new Date().toISOString());
    let host=body;
    if(body.querySelector('.daypage')){
      let pg=body.querySelector('.daypage[data-day="day-'+md.replace('/','-')+'"]');
      if(!pg){pg=document.createElement('div');pg.className='daypage';pg.dataset.day='day-'+md.replace('/','-');body.appendChild(pg);}
      host=pg;
    }
    const g=ensureGroup(host,md,kick);
    g.grid.appendChild(card);
    const n=g.grid.querySelectorAll('.card').length;
    const sp=g.head.querySelector('span');
    if(sp) sp.textContent=n+' 場';
  }
  function toast(txt){
    let t=document.getElementById('utoast');
    if(!t){t=document.createElement('div');t.id='utoast';document.body.appendChild(t);
      const s=document.createElement('style');s.textContent='#utoast{position:fixed;left:50%;transform:translateX(-50%);bottom:18px;z-index:99;background:var(--card,#12232b);color:var(--ink,#d8e6e6);border:1px solid var(--line,#2a4149);border-radius:10px;padding:8px 14px;font-size:13px;box-shadow:0 4px 16px rgba(0,0,0,.35);opacity:0;transition:opacity .4s;pointer-events:none}#utoast.show{opacity:1}@keyframes uflash{0%{box-shadow:0 0 0 2px rgba(76,155,161,.9)}100%{box-shadow:0 0 0 2px transparent}}.card.uflash{animation:uflash 1.6s ease-out 1}';document.head.appendChild(s);}
    t.textContent=txt;t.classList.add('show');
    clearTimeout(t._h);t._h=setTimeout(()=>t.classList.remove('show'),4000);
  }
  function setCard(c,d){
    const pill=c.querySelector('.pill,.pass');
    if(pill&&d.minute!=null&&d.minute!==''&&pill.textContent!==d.minute){pill.textContent=d.minute;}
    if(d.sh!=null&&d.sh>=0){
      const sb=c.querySelectorAll('.score b');
      if(sb.length===2&&(sb[0].textContent!==String(d.sh)||sb[1].textContent!==String(d.sa))){
        sb[0].textContent=d.sh;sb[1].textContent=d.sa;
        c.classList.remove('uflash');void c.offsetWidth;c.classList.add('uflash');
      }
    }
  }
  function ensureScore(c,d){
    if(!c||d.sh==null||d.sh<0) return;
    if(c.querySelector('.score')) return;
    const vs=c.querySelector('.vs');
    if(vs) vs.outerHTML='<div class="score"><b>'+d.sh+'</b><i>–</i><b>'+d.sa+'</b></div>';
  }
  async function poll(){
    try{
      const r=await fetch('data.json?t='+Date.now(),{cache:'no-store'});
      if(!r.ok) return;
      const rows=await r.json();
      rows.forEach(d=>DATA.set(d.id,d));
      let nScore=0,nMove=0,nLive=0;
      for(const d of rows){
        const c=document.querySelector('.card[data-id="'+d.id+'"]');
        if(!c) continue;
        const s=snap[d.id]||{status:c.dataset.status};
        if(d.status===s.status){ if(d.status==='live'){ensureScore(c,d);} setCard(c,d); if(d.status==='live') snap[d.id]={status:d.status,minute:d.minute,sh:d.sh,sa:d.sa}; continue; }
        // 狀態變咗：pre→live→ft/post（或延賽 pre→post）
        if(d.status==='live'&&s.status==='pre'){
          c.dataset.status='live';
          ensureScore(c,d);
          setCard(c,d);
          const pl=c.querySelector('.pill,.pass');
          if(pl){pl.className='pill live';pl.innerHTML='<em></em>'+String(d.minute||'比賽中').replace(/[<>]/g,'');}
          // 搬去「比賽中」分頁（之前只換 pill，卡仍然留喺未開賽 → 分頁永遠見唔到）
          placeCard(liveBody,c,d.kickoff||'');
          snap[d.id]={status:'live',minute:d.minute,sh:d.sh,sa:d.sa}; nLive++;
        } else if(d.status==='ft'||d.status==='post'){
          c.dataset.status=d.status;
          const pl=c.querySelector('.pill,.pass');
          if(pl){pl.className='pill ft';pl.textContent=d.status==='post'?(d.minute||'延賽'):'完場';}
          c.querySelector('.score')?.remove();
          if(d.sh!=null&&d.sh>=0&&!c.querySelector('.score')){
            c.querySelector('.teams')?.insertAdjacentHTML('beforeend','<div class="score"><b>'+d.sh+'</b><i>–</i><b>'+d.sa+'</b></div>');
          }
          // 搬去已完場分頁：優先入對應日子，否則加喺尾
          if(ftBody){
            placeCard(ftBody,c,d.kickoff||'');
            dpages.forEach(p=>p.classList.toggle('off',!searchingNow()&&p.dataset.day!==(curDay||p.dataset.day)));
          }
          snap[d.id]={status:d.status,minute:d.minute,sh:d.sh,sa:d.sa}; nMove++;
        } else { snap[d.id]={status:d.status,minute:d.minute,sh:d.sh,sa:d.sa}; }
      }
      // tab 計數重算
      const cnt={pre:0,live:0,ft:0};
      document.querySelectorAll('.card').forEach(c=>{const st=c.dataset.status;cnt[st==='post'?'ft':st]++;});
      tabs.forEach(t=>{const b=t.querySelector('b');if(b&&cnt[t.dataset.tab]!=null)b.textContent=cnt[t.dataset.tab];});
      tidy();
      if(nScore) toast('更新咗 '+nScore+' 場比分');
      if(nLive) toast(nLive+' 場開賽，搬咗去「比賽中」分頁');
      if(nMove) toast(nMove+' 場已完場，搬咗去「已完場」分頁');
      if(wOpen) wRender();
    }catch(e){/* 靜默：下一輪再試 */}
  }
  function searchingNow(){const qq=document.getElementById('q');return !!((qq&&qq.value&&qq.value.trim())||comp!=='all');}
  // ── 關注清單（localStorage，無帳號；清單只喺本機）──────────────────────
  const WKEY='md_watch_v1';
  const WSET=new Set();
  try{(JSON.parse(localStorage.getItem(WKEY)||'[]')||[]).forEach(x=>{if(typeof x==='string')WSET.add(x);});}catch(e){}
  const DATA=new Map();
  const wmodal=document.getElementById('wmodal');
  const wbody=document.getElementById('wbody');
  const wsum=document.getElementById('wsum');
  let wOpen=false;
  function wSave(){try{localStorage.setItem(WKEY,JSON.stringify([...WSET]));}catch(e){} try{pushSync();}catch(e){}}
  function esc2(s){return String(s==null?'':s).replace(/[<>&"]/g,c=>({'<':'&lt;','>':'&gt;','&':'&amp;','"':'&quot;'}[c]));}
  function markStars(){
    document.querySelectorAll('.star').forEach(b=>{
      const on=WSET.has(b.dataset.watch);
      b.classList.toggle('on',on);
      b.setAttribute('aria-pressed',on?'true':'false');
      b.title=on?'取消關注':'關注呢場';
      b.textContent=on?'★':'☆';
    });
    const c=document.getElementById('wcount'); if(c) c.textContent=WSET.size;
  }
  function wStatusText(d){
    if(!d) return '';
    if(d.status==='live') return '比賽中'+(d.minute?' '+String(d.minute):'');
    if(d.status==='ft') return '完場';
    if(d.status==='post') return String(d.minute||'延賽');
    return '未開賽';
  }
  function wScoreText(d){
    if(!d) return '';
    if((d.status==='ft'||d.status==='live')&&d.sh!=null&&d.sh>=0) return '<b>'+d.sh+'</b> – '+d.sa;
    return '';
  }
  function wStatLine(tags){
    const html=memoStatLine(tags);
    return html?html.replace(/^<div class="memo-stat-line">/,'').replace(/<\/div>\s*$/,''):'';
  }
  /* ── 通知（Web Push）：清單喺瀏覽器，訂閱送後端 push_subs ── */
  const PKEY="md_push_end_v1";
  const wpush=document.getElementById("wpush");
  const wpushmsg=document.getElementById("wpushmsg");
  function wPushMsg(t){ if(wpushmsg) wpushmsg.textContent=t||""; }
  function b64ToU8(b){ const s=atob(String(b).replace(/-/g,"+").replace(/_/g,"/")); const a=new Uint8Array(s.length); for(let i=0;i<s.length;i++) a[i]=s.charCodeAt(i); return a; }
  const isiOS=/iP(hone|ad|od)/.test(navigator.userAgent||"");
  const standalone=(window.matchMedia&&window.matchMedia("(display-mode: standalone)").matches)||navigator.standalone===true;
  function pushSupported(){ return ("serviceWorker" in navigator)&&("PushManager" in window)&&("Notification" in window); }
  function pushUI(on){
    if(!wpush) return;
    wpush.classList.toggle("on",!!on);
    wpush.textContent=on?"🔔 已開啟通知":"🔔 開啟通知";
  }
  async function pushState(){
    if(!pushSupported()){ if(wpush) wpush.disabled=true; wPushMsg("呢個瀏覽器唔支援網頁通知。"); return; }
    if(isiOS&&!standalone){ if(wpush) wpush.disabled=true; wPushMsg("iPhone／iPad 要先「分享 → 加入主畫面」，再由主畫面個圖示開啟，先收得到通知。"); return; }
    try{
      const reg=await navigator.serviceWorker.getRegistration();
      const sub=reg&&await reg.pushManager.getSubscription();
      if(wpush) wpush.disabled=false;
      pushUI(!!sub);
      wPushMsg(sub?"已開啟：關注嘅場開賽／入球／完場／標籤變會通知你。":"");
    }catch(e){ wPushMsg("狀態讀取失敗："+(e&&e.message||e)); }
  }
  async function pushEnable(){
    try{
      if(!pushSupported()){ wPushMsg("呢個瀏覽器唔支援網頁通知。"); return; }
      if(isiOS&&!standalone){ wPushMsg("請先「分享 → 加入主畫面」，再由主畫面開啟。"); return; }
      if(Notification.permission==="denied"){ wPushMsg("你已封鎖通知，要去瀏覽器設定重新允許。"); return; }
      const perm=await Notification.requestPermission();
      if(perm!=="granted"){ wPushMsg("未允許通知。"); return; }
      const reg=await navigator.serviceWorker.register("sw.js");
      await navigator.serviceWorker.ready;
      const k=await (await fetch(MEMO_API+"/watch?a=key",{cache:"no-store"})).json();
      if(!k||!k.key){ wPushMsg("伺服器未設定推送金鑰。"); return; }
      const sub=await reg.pushManager.subscribe({userVisibleOnly:true,applicationServerKey:b64ToU8(k.key)});
      const j=sub.toJSON();
      await fetch(MEMO_API+"/watch?a=sub",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({subscription:{endpoint:j.endpoint,keys:j.keys},ids:[...WSET]})});
      try{ localStorage.setItem(PKEY,j.endpoint); }catch(e){}
      await pushState();
    }catch(e){ wPushMsg("開啟失敗："+(e&&e.message||e)); }
  }
  async function pushSync(){
    try{
      const ep=localStorage.getItem(PKEY)||"";
      if(!ep) return;
      await fetch(MEMO_API+"/watch?a=sync",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({endpoint:ep,ids:[...WSET]})});
    }catch(e){}
  }
  async function pushDisable(){
    try{
      const ep=localStorage.getItem(PKEY)||"";
      const reg=await navigator.serviceWorker.getRegistration();
      const sub=reg&&await reg.pushManager.getSubscription();
      if(sub) await sub.unsubscribe();
      if(ep) await fetch(MEMO_API+"/watch?a=del",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({endpoint:ep})});
      try{ localStorage.removeItem(PKEY); }catch(e){}
      await pushState();
    }catch(e){ wPushMsg("關閉失敗："+(e&&e.message||e)); }
  }
  if(wpush) wpush.addEventListener("click",()=>{ const on=wpush.classList.contains("on"); if(on) pushDisable(); else pushEnable(); });
  // ── 隊徽（熱鏈 FotMob，本機唔存圖）：crests.json 只有「隊名→team id」 ──
  const CREST={}; let crestReady=false;
  function crestUrl(id){ return "https://images.fotmob.com/image_resources/logo/teamlogo/"+id+".png"; }
  function crestDecorate(root){
    if(!crestReady||!root||!root.querySelectorAll) return;
    root.querySelectorAll(".team .name, .wteam").forEach(sp=>{
      if(sp.dataset.crest) return;
      const name=String(sp.textContent||"").trim();
      // 文字包一層 .nm：長隊名在自己的框內 ellipsis，隊徽用 flex:0 0 auto 唔會被壓
      if(sp.appendChild && !(sp.firstChild && sp.firstChild.className==="nm")){
        const nm=document.createElement("span");
        nm.className="nm"; nm.textContent=name;
        sp.textContent=""; sp.appendChild(nm);
      }
      const id=CREST[name];
      if(!id){ sp.dataset.crest="0"; return; }
      const cl=sp.classList, pn=sp.parentNode;
      const isAway=!!(cl&&cl.contains&&cl.contains("away")||(pn&&pn.classList&&pn.classList.contains&&pn.classList.contains("away")));
      const img=document.createElement("img");
      img.className=isAway?"crest crest-l":"crest crest-r";
      img.src=crestUrl(id); img.alt="";
      img.loading="lazy"; img.decoding="async"; img.width=20; img.height=20;
      try{ img.referrerPolicy="no-referrer"; }catch(e){}
      img.onerror=function(){ try{ this.remove(); }catch(e2){ this.style.display="none"; } };
      sp.dataset.crest="1";
      fitName(sp);
      // 主隊（左邊那格）：徽章放隊名右邊，貼住中間；客隊：放隊名左邊，一樣貼住中間
      if(!sp.insertBefore) return;
      if(isAway) sp.insertBefore(img, sp.firstChild);
      else if(sp.appendChild) sp.appendChild(img);
    });
  }
  // 長隊名：先縮字級（17.5→13.5），縮到盡都放唔落才 ellipsis，避免撞到比分
  function fitName(sp){
    const nm=sp.querySelector && sp.querySelector(".nm");
    if(!nm||typeof nm.scrollWidth!=="number"||!nm.clientWidth) return;
    const base=(sp.classList&&sp.classList.contains&&sp.classList.contains("wteam"))?13.5:17.5;
    let size=base;
    const min=base-4;
    for(let k=0;k<5 && nm.scrollWidth>nm.clientWidth+1 && size>min;k++){
      size=Math.max(min,size-1);
      sp.style.fontSize=size+"px";
    }
  }
  const COMP={};
  async function compLoad(){
    try{
      const r=await fetch("comps.json",{cache:"no-cache"});
      if(!r.ok) return;
      const j=await r.json(); const m=(j&&j.map)||{};
      Object.keys(m).forEach(k=>{ COMP[k]=String(m[k]); });
      if(wOpen) wRender();
    }catch(e){}
  }
  async function crestLoad(){
    try{
      const r=await fetch("crests.json",{cache:"no-cache"});
      if(!r.ok) return;
      const j=await r.json();
      const m=(j&&j.map)||{};
      Object.keys(m).forEach(k=>{ CREST[k]=String(m[k]); });
      crestReady=true;
      crestDecorate(document);
      if(wOpen) wRender();
    }catch(e){}
  }
  function wRender(){
    if(!wbody) return;
    const rows=[...WSET].map(id=>({id:id,d:DATA.get(id)})).filter(r=>r.d);
    const rank=s=>s==='live'?0:(s==='pre'?1:2);
    rows.sort((a,b)=>rank(a.d.status)-rank(b.d.status)||String(a.d.kickoff||'').localeCompare(String(b.d.kickoff||'')));
    if(wsum) wsum.textContent=WSET.size?('共 '+WSET.size+' 場'):'';
    if(!rows.length){
      wbody.innerHTML='<div class="wempty">未關注任何賽事。<br>喺每張卡右上角按 <b>☆</b> 就會加入呢個清單；再按一次 <b>★</b> 取消；或喺關注頁每張卡右上角按 <b>✕</b>。</div>';
      return;
    }
    wbody.innerHTML=rows.map(r=>{
      const d=r.d, m=(MEMO_ALL[r.id]||memoCache[r.id]), tags=(m&&m.tags&&m.tags.length)?m.tags:null;
      const line=tags?wStatLine(tags.join("·")):"";
      const kd=String(d.kickoff||""), md=mdOf(kd), hm=kd.slice(11,16);
      const st=d.status;
      const pill=st==="live"?"pill live":(st==="ft"?"pill ft":"pill soon");
      const sc=wScoreText(d);
      const tagHtml=tags
        ? tags.map(t=>"<span>#"+String(t).replace(/[<>&]/g,"")+"</span>").join("")
        : "<span>未有 memo 標籤</span>";
      return "<div class=\"wcard\" data-jump=\""+r.id+"\" tabindex=\"0\">"
        + "<div class=\"wcard-top\">"
        + "<span class=\"wt\">"+labelOf(kd,md)+" "+hm+"</span>"
        + (d.comp?"<span class=\"wcomp\">"+esc2(COMP[d.comp]||d.comp)+"</span>":"")
        + "<span class=\""+pill+"\">"+(st==="live"?"<em></em>":"")+esc2(wStatusText(d))+"</span>"
        + "<button class=\"wdel\" type=\"button\" data-del=\""+r.id+"\" aria-label=\"取消關注\" title=\"取消關注\">✕</button>"
        + "</div>"
        + "<div class=\"wcard-mid\">"
        + "<span class=\"wteam\">"+esc2(d.home)+"</span>"
        + "<span class=\"wsc\">"+(sc||"<i>vs</i>")+"</span>"
        + "<span class=\"wteam away\">"+esc2(d.away)+"</span>"
        + "</div>"
        + "<div class=\"wcard-tags\">"+tagHtml+"</div>"
        + (line?"<div class=\"wcard-stat\">"+line+"</div>":"")
        + "</div>";
    }).join("");
    crestDecorate(wbody);
  }
  async function wLoadData(){
    try{
      const r=await fetch('data.json?t='+Date.now(),{cache:'no-store'});
      if(!r.ok) return;
      const rows=await r.json();
      rows.forEach(d=>DATA.set(d.id,d));
    }catch(e){}
  }
  async function wOpenPanel(){
    wOpen=true;
    if(wmodal) wmodal.classList.add('show');
    document.documentElement.style.overflow='hidden';
    markStars(); wRender();
    if(!DATA.size){ await wLoadData(); wRender(); }
  }
  function wClosePanel(){
    wOpen=false;
    if(wmodal) wmodal.classList.remove('show');
    document.documentElement.style.overflow='';
  }
  function wJump(id){
    const d=DATA.get(id), c=document.querySelector('.card[data-id="'+id+'"]');
    wClosePanel();
    if(!c) return;
    const st=c.dataset.status;
    showTab(st==='live'?'live':(st==='pre'?'pre':'ft'));
    const qq=document.getElementById('q'); if(qq) qq.value='';
    comp='all'; chips.forEach(x=>x.classList.remove('on')); if(chips[0]) chips[0].classList.add('on');
    if(d&&dpages.length){ const md=mdOf(d.kickoff||''); if(md) showDay('day-'+md.replace('/','-')); }
    apply();
    c.style.display='';
    c.scrollIntoView({behavior:'smooth',block:'center'});
    c.classList.remove('uflash'); void c.offsetWidth; c.classList.add('uflash');
  }
  const wbtn=document.getElementById('watchbtn');
  if(wbtn) wbtn.addEventListener('click',()=>{ if(wOpen) wClosePanel(); else wOpenPanel(); });
  const wcloseBtn=document.getElementById('wclose');
  if(wcloseBtn) wcloseBtn.addEventListener('click',wClosePanel);
  if(wmodal) wmodal.addEventListener('click',ev=>{ if(ev.target===wmodal) wClosePanel(); });
  document.addEventListener('keydown',ev=>{
    if(ev.key==='Escape'&&wOpen){ wClosePanel(); return; }
    if(ev.key==='Enter'||ev.key===' '){
      if(ev.target&&ev.target.closest&&ev.target.closest('.wdel')) return;
      const card=ev.target&&ev.target.closest?ev.target.closest('.wcard'):null;
      if(card&&card.dataset.jump){ ev.preventDefault(); wJump(card.dataset.jump); }
    }
  });
  document.addEventListener('click',ev=>{
    const st=ev.target.closest?ev.target.closest('.star'):null;
    if(st){ ev.preventDefault(); ev.stopPropagation();
      const id=st.dataset.watch;
      if(WSET.has(id)) WSET.delete(id); else WSET.add(id);
      wSave(); markStars();
      toast(WSET.has(id)?'已加入關注':'已取消關注');
      if(wOpen) wRender();
      return;
    }
    const del=ev.target.closest?ev.target.closest('.wdel'):null;
    if(del){ ev.preventDefault(); ev.stopPropagation();
      const did=del.dataset.del;
      WSET.delete(did); wSave(); markStars();
      toast('已取消關注');
      if(wOpen) wRender();
      return;
    }
    const row=ev.target.closest?ev.target.closest('.wcard'):null;
    if(row&&row.dataset.jump) wJump(row.dataset.jump);
  });
  markStars();
  if(location.hash==='#watch') wOpenPanel();
  else if(/^#m-\d+/.test(location.hash)){ const _mid=location.hash.slice(3); (async()=>{ await wOpenPanel(); const el=document.querySelector('.wcard[data-jump="'+_mid+'"]'); if(el) el.scrollIntoView({block:'center'}); })(); }
  setTimeout(pushState,600);
  crestLoad();
  compLoad();
  // 頁腳版本：一眼睇得出載入咗邊個版本（分辨舊快取）
  fetch("build.txt",{cache:"no-store"}).then(r=>r.ok?r.text():"").then(t=>{
    const el=document.getElementById("bver"); if(el&&t) el.textContent="v "+t.trim().slice(0,16).replace("T"," ");
  }).catch(()=>{});
  setInterval(poll,60000);
})();
"#;

// 已完場／異常終局而冇預測 → 唔應該再顯示（冇得補跑；ghost note 會變假承諾）
fn has_pred_none(r: &Value) -> bool {
    let s = text(r, &["status"]);
    if s != "ft" && s != "post" {
        return false;
    }
    let p = r.get("pred").cloned().unwrap_or(Value::Null);
    p.get("base").and_then(|b| b.get("h")).is_none()
}

fn tab_body(rows: &[Value], status: &str, empty_msg: &str) -> String {
    let mut list: Vec<&Value> = rows
        .iter()
        .filter(|r| {
            let s = text(r, &["status"]);
            // post（延賽／腰斬／中斷／取消／待定）歸「已完場」分頁（用戶 m05969）
            let ok = if status == "ft" { s == "ft" || s == "post" } else { s == status };
            // 已完場／異常終局而冇預測 → 收埋（冇得補跑，ghost note 會變假承諾）
            ok && !has_pred_none(r)
        })
        .collect();
    if status == "ft" {
        list.reverse();
    }
    if list.is_empty() {
        return format!("<div class=\"panel empty\">{}</div>", esc(empty_msg));
    }
    // 依開球日分組（list 已按 kick 排序）
    let mut days: Vec<(String, String, Vec<&Value>)> = Vec::new(); // (md, 顯示, 卡)
    for r in list {
        let kick = text(r, &["kick"]);
        let (md, _) = kick_parts(&kick);
        let wd = weekday(&md).to_string();
        let key = format!("{} 週{}", md, wd);
        match days.last_mut() {
            Some((m, _, g)) if *m == md => g.push(r),
            _ => days.push((md, key, vec![r])),
        }
    }
    let mut out = String::new();
    // 已完場多於一日 → 頂部 sticky 日子導航（用戶要求：分日子查賽事）
    if status == "ft" && days.len() > 1 {
        out.push_str("<div class=\"daybar\">");
        for (md, key, g) in &days {
            out.push_str(&format!(
                "<button class=\"daynav\" data-day=\"day-{}\">{}<em>{}</em></button>",
                md.replace('/', "-"),
                esc(&key.replace('週', "")),
                g.len()
            ));
        }
        out.push_str("</div>");
    }
    for (md, key, group) in &days {
        // 已完場：一日一頁（用戶要求分頁，唔係一整頁滾）；其餘分頁照舊平鋪
        if status == "ft" && days.len() > 1 {
            out.push_str(&format!("<div class=\"daypage\" data-day=\"day-{}\">", md.replace('/', "-")));
        }
        out.push_str(&format!(
            "<div class=\"dayhead\" id=\"day-{}\"><b>{}</b><span>{} 場</span></div><div class=\"grid\">",
            md.replace('/', "-"),
            esc(key),
            group.len()
        ));
        for r in group {
            out.push_str(&card(r));
        }
        out.push_str("</div>");
        if status == "ft" && days.len() > 1 {
            out.push_str("</div>");
        }
    }
    out
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut fixtures = PathBuf::from("../../web/fixtures.json");
    let mut registry = PathBuf::from("../../fb/registry.json");
    let mut out = PathBuf::from("dist/index.html");
    let mut keep_all = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--fixtures" => fixtures = PathBuf::from(args.next().unwrap_or_default()),
            "--registry" => registry = PathBuf::from(args.next().unwrap_or_default()),
            "--out" => out = PathBuf::from(args.next().unwrap_or_default()),
            "--all" => keep_all = true,
            "-h" | "--help" => {
                println!("matchdeck --fixtures <json> --registry <json> --out <html> [--all]");
                return;
            }
            _ => {}
        }
    }

    // 讀賽程總表
    let fx: Value = match fs::read_to_string(&fixtures).ok().and_then(|t| json::parse(&t).ok()) {
        Some(v) => v,
        None => {
            eprintln!("讀不到 {}", fixtures.display());
            Value::Null
        }
    };
    let mut rows: Vec<Value> = fx
        .get("rows")
        .and_then(|x| x.as_arr())
        .map(|a| a.to_vec())
        .unwrap_or_default();
    rows.sort_by(|a, b| text(a, &["kick"]).cmp(&text(b, &["kick"])));
    let gen_at = text(&fx, &["generated"]);

    if !keep_all {
        // 已完場保留 336 小時（14 日）：用戶要求已完場紀錄唔好咁快消失（對帳用）；
        // 7 日窗曾喺週日令 27 場上週六嘅賽事一夜消失，改 14 日避免週期性缺口
        let cut = now_secs() - 336 * 3600;
        rows.retain(|r| {
            let s = text(r, &["status"]);
            if s != "ft" && s != "post" {
                return true;
            }
            // 有覆盤 memo 嘅場次永遠保留（用戶寫咗 memo 就要見到張卡）
            if r.get("memo").and_then(|x| x.as_bool()).unwrap_or(false) {
                return true;
            }
            epoch_secs(&text(r, &["kick"])).map(|k| k > cut).unwrap_or(true)
        });
    }

    let n_pre = rows.iter().filter(|r| text(r, &["status"]) == "pre").count();
    let n_live = rows.iter().filter(|r| text(r, &["status"]) == "live").count();
    let n_ft = rows.iter().filter(|r| { let s = text(r, &["status"]); (s == "ft" || s == "post") && !has_pred_none(r) }).count();
    let n_pred = rows
        .iter()
        .filter(|r| r.get("pred").map(|p| p.get("base").is_some()).unwrap_or(false))
        .count();
    let n_comps = rows
        .iter()
        .map(|r| text(r, &["comp"]))
        .collect::<std::collections::BTreeSet<_>>()
        .len();

    // registry：聯賽接駁表
    let reg: Value = fs::read_to_string(&registry)
        .ok()
        .and_then(|t| json::parse(&t).ok())
        .unwrap_or(Value::Null);
    let mut comps: Vec<(String, u64, bool, String)> = Vec::new();
    if let Value::Obj(m) = reg.clone() {
        for (k, v) in m {
            if k.starts_with('_') {
                continue;
            }
            let fm = num(&v, &["fotmob"], 0.0) as u64;
            if fm == 0 {
                continue;
            }
            let ver = v.get("verified").and_then(|x| x.as_bool()).unwrap_or(false);
            comps.push((k, fm, ver, text(&v, &["name"])));
        }
    }
    comps.sort_by(|a, b| a.0.cmp(&b.0));

    // 篩選 chips：只列今日有賽程的聯賽
    let mut active: Vec<String> = rows.iter().map(|r| text(r, &["comp"])).collect();
    active.sort();
    active.dedup();
    let chips = std::iter::once(String::from(
        "<button class=\"fchip on\" data-filter=\"all\">全部</button>",
    ))
    .chain(active.iter().map(|c| {
        let n = rows.iter().filter(|r| text(r, &["comp"]) == *c).count();
        format!(
            "<button class=\"fchip\" data-filter=\"{}\">{}<em>{}</em></button>",
            esc(c),
            esc(c),
            n
        )
    }))
    .collect::<String>();

    let cov: String = comps
        .iter()
        .map(|(c, fm, ver, name)| {
            let n = rows.iter().filter(|r| text(r, &["comp"]) == *c).count();
            let v = if *ver { "✓" } else { "·" };
            format!(
                "<span title=\"FotMob league {}\">{} <b>{}</b>{} {}</span>",
                fm,
                esc(c),
                fm,
                v,
                if n > 0 {
                    format!("· {} 場", n)
                } else {
                    esc(name)
                }
            )
        })
        .collect();

    let rows_tbl = [
        ("Step 1 定量基線", "時序 Elo（K=20、主場 +60）＋時間衰減攻防擬合（半衰期 180 天、先驗 K=2）", "done"),
        ("Step 1b 賽果數據", "FotMob 賽果 2023–2026，跨年制聯賽自動處理 2023/2024 編碼", "done"),
        ("Step 1c 傷停／首發", "FotMob squad.injury ＋ matchDetails.lineup（T-60 取預測、T-25 取官方）", "done"),
        ("Step 1d 國際賽先驗", "成年國家隊改用 eloratings.net 歷史 Elo 時間序列（247 隊，取開賽前一日評分，零 look-ahead）直接定 λ；青年隊同對唔到嘅隊回落到模型", "done"),
        ("Step 2 多智能體共識", "統計／戰術／情境／風險四代理，權重 0.15/0.05/0.20/0.60；控球取向已接入，但 3,975 場回測顯示增益 ≈ 0，故壓至 0.05；出 λ 對數調整＋和局偏移（±3pp）", "done"),
        ("Step 3 盃賽 Monte Carlo", "聯賽場景不需要；盃賽待補", "todo"),
        ("Step 4 置信度", "已停用：星級依樣本量與首發狀態，屬非量化欄位，前後台計算保留但唔顯示", "todo"),
        ("Step 5 校準", "逐聯賽 walk-forward logistic（8 摺、out-of-sample）＋ pooled 混合，cal 為參考層不覆蓋 fin", "done"),
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

    let calib_rows = {
        let mut out: Vec<(f64, String)> = Vec::new();
        if let Some(cb) = fx.get("calib").and_then(|x| x.as_arr()) {
            for r in cb.iter() {
                if r.get("insufficient").and_then(|v| v.as_bool()).is_some() { continue; }
                let name = text(r, &["name"]);
                if name.is_empty() { continue; }
                let n = num(r, &["n"], 0.0);
                let ab = num(r, &["acc_base"], 0.0) * 100.0;
                let ac = num(r, &["acc_cal"], 0.0) * 100.0;
                let bb = num(r, &["brier_base"], 0.0);
                let bc = num(r, &["brier_cal"], 0.0);
                let d = bc - bb;
                let cls = if d < -0.0005 { "up" } else if d > 0.0005 { "down" } else { "" };
                out.push((n, format!(
                    "<tr><td>{}</td><td>{:.0}</td><td>{:.1}% → <b>{:.1}%</b></td><td>{:.4} → <b>{:.4}</b></td><td class=\"{}\">{:+.4}</td></tr>",
                    esc(&name), n, ab, ac, bb, bc, cls, d)));
            }
        }
        out.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        if let Some(pl) = fx.get("pooled") {
            let n = num(pl, &["n_samples"], 0.0);
            let ab = num(pl, &["metrics", "acc_base"], 0.0) * 100.0;
            let ac = num(pl, &["metrics", "acc_cal"], 0.0) * 100.0;
            let bb = num(pl, &["metrics", "brier_base"], 0.0);
            let bc = num(pl, &["metrics", "brier_cal"], 0.0);
            out.insert(0, (f64::MAX, format!(
                "<tr style=\"background:rgba(76,155,161,.08)\"><td><b>pooled（全部聯賽）</b></td><td>{:.0}</td><td>{:.1}% → <b>{:.1}%</b></td><td>{:.4} → <b>{:.4}</b></td><td class=\"up\">{:+.4}</td></tr>",
                n, ab, ac, bb, bc, bc - bb)));
        }
        out.into_iter().map(|(_, s)| s).collect::<Vec<_>>().join("")
    };
    let body_pre = tab_body(&rows, "pre", "48 小時內暫無未開賽場次。");
    let body_live = tab_body(&rows, "live", "目前沒有進行中的賽事。");
    let body_ft = tab_body(&rows, "ft", "最近 14 日沒有完場紀錄。");

    let now = time_now();
    let gen_note = if gen_at.is_empty() { now.clone() } else { gen_at.clone() };

    // 頁底三塊與頁腳細項：用戶要求收埋（m05921）；代碼保留，一行開返
    let meta_html = if SHOW_META {
        format!(
            r#"<section class="block">
    <h2>流程合規狀態</h2>
    <div class="panel"><table>
      <thead><tr><th>步驟</th><th>現況</th><th>狀態</th></tr></thead>
      <tbody>{rows_tbl}</tbody>
    </table></div>
  </section>

  <section class="block">
    <h2>Step 5 校準回測</h2>
    <div class="panel">
      <p class="hint" style="margin:0 0 10px;font-size:12.5px;color:#5c8b8f">walk-forward logistic：每摺只用該摺之前的賽果訓練，特徵 [P_h, P_d, P_a, Elo差, lnλ主, lnλ客]。下表為各聯賽的樣本數與「基線 vs 校準」命中率、Brier。Brier 越低越好。</p>
      <table class="cal-table"><thead><tr><th>聯賽</th><th>樣本</th><th>命中率</th><th>Brier</th><th>Brier 改善</th></tr></thead>
      <tbody>{calib_rows}</tbody></table>
    </div>
  </section>

  <section class="block">
    <h2>聯賽接駁</h2>
    <div class="panel"><div class="cov">{cov}</div></div>
  </section>"#,
            rows_tbl = rows_tbl,
            calib_rows = calib_rows,
            cov = cov,
        )
    } else {
        String::new()
    };
    let footer_html = if SHOW_FOOT_META {
        format!(
            r#"<footer>
    <b>資料時間</b>　{gen_note}　·　頁面生成 {now}<br>
    <b>範圍</b>　未開賽只列 48 小時內；已完場保留 14 日（分日子顯示）。<br>
    <b>免責</b>　本頁為概率模型輸出與數據整理，不構成任何投注建議或誘因。概率為估計值，非保證。<br>
    <b>資料來源</b>　香港賽馬會公開賠率（僅用作選場與賽後對比）· FotMob 賽果／傷停／陣容／即時比分。
  </footer>"#,
            gen_note = esc(&gen_note),
            now = now,
        )
    } else {
        String::from("<footer><b>免責</b>　本頁為概率模型輸出與數據整理，不構成任何投注建議或誘因。概率為估計值，非保證。　·　<a href=\"memo.html\">覆盤 memo 標籤勝率</a>　·　<span class=\"bver\" id=\"bver\">v—</span></footer>")
    };

    // 頁頂 tagline 與統計五格：用戶要求收埋；代碼保留，一行開返
    let tagline_html = if SHOW_TAGLINE {
        String::from(r#"<p class="tagline">香港賽馬會賽程 <b>×</b> FotMob 賽果與傷停 <b>×</b> Elo＋泊松／負二項</p>"#)
    } else {
        String::new()
    };
    let stats_html = if SHOW_STATS {
        format!(
            r#"<div class="stats">
      <div class="stat"><b>{n_pre}</b><i>未開賽</i></div>
      <div class="stat"><b>{n_live}</b><i>比賽中</i></div>
      <div class="stat"><b>{n_ft}</b><i>已完場</i></div>
      <div class="stat"><b>{n_pred}</b><i>已出預測</i></div>
      <div class="stat"><b>{n_comps}</b><i>聯賽</i></div>
    </div>"#,
            n_pre = n_pre,
            n_live = n_live,
            n_ft = n_ft,
            n_pred = n_pred,
            n_comps = n_comps,
        )
    } else {
        String::new()
    };

    let html = format!(
        r##"<!doctype html>
<html lang="zh-Hant">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>賽前預測台 · 霧青</title>
<meta name="description" content="香港賽馬會賽程 × FotMob 數據 × Elo／泊松模型的賽前預測看板">
<meta name="theme-color" content="#0b1220">
<link rel="manifest" href="manifest.json">
<link rel="icon" type="image/svg+xml" href="doge.svg">
<link rel="apple-touch-icon" href="doge-192.png">
<meta name="apple-mobile-web-app-capable" content="yes">
<meta name="mobile-web-app-capable" content="yes">
<meta name="apple-mobile-web-app-status-bar-style" content="black-translucent">
<meta name="apple-mobile-web-app-title" content="MatchDeck">
<style>{css}</style>
</head>
<body>
<div class="wrap">
  <header class="hero">
    <div class="mist"></div>
    <h1>賽前預測台<span class="dot"></span></h1>
    {tagline_html}{stats_html}
  </header>

  <div class="toolbar">
    <input id="q" class="search" placeholder="搜尋球隊或聯賽…" autocomplete="off">
    <button id="watchbtn" class="watchbtn" type="button" aria-haspopup="dialog">★ 關注 <b id="wcount">0</b></button>
    <div class="chips" style="display:none">{chips}</div>   <!-- 聯賽過濾列：用戶要求收埋；要恢復刪 style 即可 -->
  </div>

  <nav class="tabs" role="tablist">
    <button class="tab on" data-tab="pre">未開賽<b>{n_pre}</b></button>
    <button class="tab" data-tab="live">比賽中<b>{n_live}</b></button>
    <button class="tab" data-tab="ft">已完場<b>{n_ft}</b></button>
  </nav>

  <section class="tabbody" data-body="pre">{body_pre}</section>
  <section class="tabbody off" data-body="live">{body_live}</section>
  <section class="tabbody off" data-body="ft">{body_ft}</section>

  {watch_html}
  {meta_html}
  {footer_html}
</div>
<script>{js}</script>
</body>
</html>"##,
        css = CSS,
        js = JS,
        chips = chips,
        body_pre = body_pre,
        body_live = body_live,
        body_ft = body_ft,
        n_pre = n_pre,
        n_live = n_live,
        n_ft = n_ft,
        // n_pred / n_comps 已移入 stats_html 的 format!（統計五格收埋時不再需要）
        tagline_html = tagline_html,
        stats_html = stats_html,
        watch_html = WATCH_HTML,
        meta_html = meta_html,
        footer_html = footer_html,
    );

    if let Some(dir) = out.parent() {
        let _ = fs::create_dir_all(dir);
    }
    match fs::write(&out, &html) {
        Ok(_) => println!("wrote {} ({} bytes)", out.display(), html.len()),
        Err(e) => eprintln!("write failed: {e}"),
    }

    let data_path = out.parent().unwrap_or(Path::new(".")).join("data.json");
    let mut flat = String::from("[\n");
    for (i, r) in rows.iter().enumerate() {
        let p = r.get("pred").cloned().unwrap_or(Value::Null);
        let rec = Value::Obj(vec![
            ("id".into(), Value::Str(text(r, &["id"]))),
            ("comp".into(), Value::Str(text(r, &["comp"]))),
            ("kickoff".into(), Value::Str(text(r, &["kick"]))),
            ("home".into(), Value::Str(text(r, &["home"]))),
            ("away".into(), Value::Str(text(r, &["away"]))),
            ("status".into(), Value::Str(text(r, &["status"]))),
            ("minute".into(), Value::Str(text(r, &["minute"]))),
            ("sh".into(), Value::Num(num(r, &["sh"], -1.0))),
            ("sa".into(), Value::Num(num(r, &["sa"], -1.0))),
            ("base".into(), p.get("base").cloned().unwrap_or(Value::Null)),
            ("fin".into(), p.get("fin").cloned().unwrap_or(Value::Null)),
            ("handicap".into(), get(&p, &["diag", "handicap"]).cloned().unwrap_or(Value::Null)),
            ("lambda".into(), get(&p, &["diag", "lambda_adj"]).cloned().unwrap_or(Value::Null)),
            ("lineup".into(), Value::Str(text(&p, &["diag", "lineupState"]))),
        ]);
        flat.push_str("  ");
        flat.push_str(&rec.to_string());
        if i + 1 < rows.len() {
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