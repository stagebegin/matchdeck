//! 極簡 JSON 解析器（零依賴，只用 std）。
//! 只為讀本專案自產的預測檔：物件、陣列、字串、數字、布林、null。

use std::fmt;

#[derive(Clone, Debug)]
pub enum J {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    pub fn get(&self, key: &str) -> Option<&J> {
        match self {
            J::Obj(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn at(&self, idx: usize) -> Option<&J> {
        match self {
            J::Arr(a) => a.get(idx),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            J::Num(n) => Some(*n),
            J::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            J::Str(s) => s.parse::<f64>().ok(),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            J::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            J::Bool(b) => Some(*b),
            _ => None,
        }
    }
    pub fn as_arr(&self) -> Option<&Vec<J>> {
        match self {
            J::Arr(a) => Some(a),
            _ => None,
        }
    }
}

pub fn parse(src: &str) -> Result<J, String> {
    let b: Vec<char> = src.chars().collect();
    let mut i = 0usize;
    let v = parse_val(&b, &mut i)?;
    skip_ws(&b, &mut i);
    Ok(v)
}

fn skip_ws(b: &[char], i: &mut usize) {
    while *i < b.len() && matches!(b[*i], ' ' | '\t' | '\n' | '\r') {
        *i += 1;
    }
}

fn parse_val(b: &[char], i: &mut usize) -> Result<J, String> {
    skip_ws(b, i);
    if *i >= b.len() {
        return Err("unexpected end".into());
    }
    match b[*i] {
        '{' => parse_obj(b, i),
        '[' => parse_arr(b, i),
        '"' => Ok(J::Str(parse_str(b, i)?)),
        't' => {
            expect(b, i, "true")?;
            Ok(J::Bool(true))
        }
        'f' => {
            expect(b, i, "false")?;
            Ok(J::Bool(false))
        }
        'n' => {
            expect(b, i, "null")?;
            Ok(J::Null)
        }
        _ => parse_num(b, i),
    }
}

fn expect(b: &[char], i: &mut usize, lit: &str) -> Result<(), String> {
    for c in lit.chars() {
        if *i >= b.len() || b[*i] != c {
            return Err(format!("expected {lit} at {i}"));
        }
        *i += 1;
    }
    Ok(())
}

fn parse_obj(b: &[char], i: &mut usize) -> Result<J, String> {
    *i += 1; // {
    let mut out = Vec::new();
    skip_ws(b, i);
    if *i < b.len() && b[*i] == '}' {
        *i += 1;
        return Ok(J::Obj(out));
    }
    loop {
        skip_ws(b, i);
        let k = parse_str(b, i)?;
        skip_ws(b, i);
        if *i >= b.len() || b[*i] != ':' {
            return Err("expected ':'".into());
        }
        *i += 1;
        let v = parse_val(b, i)?;
        out.push((k, v));
        skip_ws(b, i);
        if *i >= b.len() {
            return Err("unterminated object".into());
        }
        match b[*i] {
            ',' => {
                *i += 1;
            }
            '}' => {
                *i += 1;
                return Ok(J::Obj(out));
            }
            _ => return Err("expected ',' or '}'".into()),
        }
    }
}

fn parse_arr(b: &[char], i: &mut usize) -> Result<J, String> {
    *i += 1; // [
    let mut out = Vec::new();
    skip_ws(b, i);
    if *i < b.len() && b[*i] == ']' {
        *i += 1;
        return Ok(J::Arr(out));
    }
    loop {
        let v = parse_val(b, i)?;
        out.push(v);
        skip_ws(b, i);
        if *i >= b.len() {
            return Err("unterminated array".into());
        }
        match b[*i] {
            ',' => {
                *i += 1;
            }
            ']' => {
                *i += 1;
                return Ok(J::Arr(out));
            }
            _ => return Err("expected ',' or ']'".into()),
        }
    }
}

fn parse_str(b: &[char], i: &mut usize) -> Result<String, String> {
    skip_ws(b, i);
    if *i >= b.len() || b[*i] != '"' {
        return Err("expected string".into());
    }
    *i += 1;
    let mut s = String::new();
    while *i < b.len() {
        let c = b[*i];
        *i += 1;
        match c {
            '"' => return Ok(s),
            '\\' => {
                if *i >= b.len() {
                    return Err("bad escape".into());
                }
                let e = b[*i];
                *i += 1;
                match e {
                    '"' => s.push('"'),
                    '\\' => s.push('\\'),
                    '/' => s.push('/'),
                    'b' => s.push('\u{8}'),
                    'f' => s.push('\u{c}'),
                    'n' => s.push('\n'),
                    'r' => s.push('\r'),
                    't' => s.push('\t'),
                    'u' => {
                        let cp = hex4(b, i)?;
                        *i += 4;
                        let ch = if (0xD800..0xDC00).contains(&cp) && *i + 5 < b.len() && b[*i] == '\\' && b[*i + 1] == 'u' {
                            // 代理對
                            let lo = {
                                *i += 2;
                                let v = hex4(b, i)?;
                                *i += 4;
                                v
                            };
                            0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00)
                        } else {
                            cp
                        };
                        s.push(char::from_u32(ch as u32).unwrap_or('\u{fffd}'));
                    }
                    other => s.push(other),
                }
            }
            _ => s.push(c),
        }
    }
    Err("unterminated string".into())
}

fn hex4(b: &[char], i: &usize) -> Result<u32, String> {
    if *i + 4 > b.len() {
        return Err("bad \\u".into());
    }
    let mut v = 0u32;
    for k in 0..4 {
        let c = b[*i + k].to_digit(16).ok_or("bad hex")?;
        v = v * 16 + c;
    }
    Ok(v)
}

fn parse_num(b: &[char], i: &mut usize) -> Result<J, String> {
    let start = *i;
    if *i < b.len() && (b[*i] == '-' || b[*i] == '+') {
        *i += 1;
    }
    while *i < b.len() && (b[*i].is_ascii_digit() || b[*i] == '.' || b[*i] == 'e' || b[*i] == 'E' || b[*i] == '-' || b[*i] == '+') {
        if (b[*i] == 'e' || b[*i] == 'E') && *i + 1 < b.len() && (b[*i + 1] == '-' || b[*i + 1] == '+') {
            *i += 2;
        } else {
            *i += 1;
        }
    }
    let s: String = b[start..*i].iter().collect();
    s.parse::<f64>().map(J::Num).map_err(|_| format!("bad number: {s}"))
}

pub fn num_str(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

impl fmt::Display for J {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            J::Null => write!(f, "null"),
            J::Bool(b) => write!(f, "{b}"),
            J::Num(n) => write!(f, "{}", num_str(*n)),
            J::Str(s) => write!(f, "\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
            J::Arr(a) => {
                write!(f, "[")?;
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            J::Obj(m) => {
                write!(f, "{{")?;
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "\"{}\":{v}", k.replace('\\', "\\\\").replace('"', "\\\""))?;
                }
                write!(f, "}}")
            }
        }
    }
}