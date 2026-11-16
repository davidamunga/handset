//! A small JSON reader and writer, for the MCP server (`mcp.rs`). The project has no JSON library and this needs
//! little: read a request, write an answer. Reading is strict (what is not JSON is an error) and bounded (a
//! deep or long document cannot use up the stack or memory), because what it reads comes from another program.

/// How deep arrays and objects may nest.
const MAX_DEPTH: usize = 64;

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    /// In the order written, which is the order read.
    Obj(Vec<(String, Json)>),
}

impl From<&str> for Json {
    fn from(s: &str) -> Json {
        Json::Str(s.to_string())
    }
}

impl From<String> for Json {
    fn from(s: String) -> Json {
        Json::Str(s)
    }
}

impl From<bool> for Json {
    fn from(b: bool) -> Json {
        Json::Bool(b)
    }
}

impl From<i64> for Json {
    fn from(n: i64) -> Json {
        Json::Num(n as f64)
    }
}

/// `obj([("a", 1.into()), ("b", "x".into())])`
pub fn obj<const N: usize>(pairs: [(&str, Json); N]) -> Json {
    Json::Obj(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

impl Json {
    pub fn parse(text: &str) -> Result<Json, String> {
        let mut p = Parser { s: text.as_bytes(), i: 0 };
        p.space();
        let v = p.value(0)?;
        p.space();
        if p.i != p.s.len() {
            return Err(p.fail("more after the value"));
        }
        Ok(v)
    }

    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// A whole number that fits: `3` and `3.0` are, `3.5` and `1e30` are not.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Json::Num(n) if n.is_finite() && n.fract() == 0.0 && n.abs() <= 9_007_199_254_740_992.0 => Some(*n as i64),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Num(n) if n.is_finite() => Some(*n),
            _ => None,
        }
    }

    #[cfg(test)]
    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }

    /// One line, no spaces: what the MCP stdio transport needs (a message may not hold a line break).
    pub fn write(&self) -> String {
        let mut out = String::new();
        self.write_to(&mut out);
        out
    }

    fn write_to(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Num(n) if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e15 => {
                out.push_str(&(*n as i64).to_string())
            }
            Json::Num(n) if n.is_finite() => out.push_str(&n.to_string()),
            // JSON has no way to write these
            Json::Num(_) => out.push_str("null"),
            Json::Str(s) => write_str(s, out),
            Json::Arr(a) => {
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    v.write_to(out);
                }
                out.push(']');
            }
            Json::Obj(pairs) => {
                out.push('{');
                for (i, (k, v)) in pairs.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_str(k, out);
                    out.push(':');
                    v.write_to(out);
                }
                out.push('}');
            }
        }
    }
}

fn write_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // the rest of the control characters, and the two line separators some readers split on
            c if (c as u32) < 0x20 || c == '\u{7f}' || c == '\u{2028}' || c == '\u{2029}' => {
                out.push_str(&format!("\\u{:04x}", c as u32))
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn fail(&self, what: &str) -> String {
        format!("not JSON: {what} at byte {}", self.i)
    }

    fn space(&mut self) {
        while matches!(self.s.get(self.i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    fn eat(&mut self, c: u8) -> bool {
        if self.s.get(self.i) == Some(&c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn word(&mut self, w: &str, v: Json) -> Result<Json, String> {
        if self.s[self.i..].starts_with(w.as_bytes()) {
            self.i += w.len();
            Ok(v)
        } else {
            Err(self.fail("unexpected word"))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, String> {
        if depth > MAX_DEPTH {
            return Err(self.fail("nested too deep"));
        }
        match self.s.get(self.i) {
            None => Err(self.fail("nothing where a value should be")),
            Some(b'n') => self.word("null", Json::Null),
            Some(b't') => self.word("true", Json::Bool(true)),
            Some(b'f') => self.word("false", Json::Bool(false)),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b'[') => {
                self.i += 1;
                let mut items = Vec::new();
                self.space();
                if self.eat(b']') {
                    return Ok(Json::Arr(items));
                }
                loop {
                    self.space();
                    items.push(self.value(depth + 1)?);
                    self.space();
                    if self.eat(b',') {
                        continue;
                    }
                    if self.eat(b']') {
                        return Ok(Json::Arr(items));
                    }
                    return Err(self.fail("expected , or ]"));
                }
            }
            Some(b'{') => {
                self.i += 1;
                let mut pairs: Vec<(String, Json)> = Vec::new();
                self.space();
                if self.eat(b'}') {
                    return Ok(Json::Obj(pairs));
                }
                loop {
                    self.space();
                    if self.s.get(self.i) != Some(&b'"') {
                        return Err(self.fail("expected a name"));
                    }
                    let key = self.string()?;
                    self.space();
                    if !self.eat(b':') {
                        return Err(self.fail("expected :"));
                    }
                    self.space();
                    let v = self.value(depth + 1)?;
                    // a repeated name: the last one counts, as most readers have it
                    pairs.retain(|(k, _)| *k != key);
                    pairs.push((key, v));
                    self.space();
                    if self.eat(b',') {
                        continue;
                    }
                    if self.eat(b'}') {
                        return Ok(Json::Obj(pairs));
                    }
                    return Err(self.fail("expected , or }"));
                }
            }
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.fail("unexpected character")),
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.i;
        self.eat(b'-');
        // no leading zeros, as JSON has it
        match self.s.get(self.i) {
            Some(b'0') => self.i += 1,
            Some(b'1'..=b'9') => {
                while matches!(self.s.get(self.i), Some(b'0'..=b'9')) {
                    self.i += 1;
                }
            }
            _ => return Err(self.fail("a number needs digits")),
        }
        if self.eat(b'.') {
            let from = self.i;
            while matches!(self.s.get(self.i), Some(b'0'..=b'9')) {
                self.i += 1;
            }
            if self.i == from {
                return Err(self.fail("digits after the point"));
            }
        }
        if matches!(self.s.get(self.i), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(self.s.get(self.i), Some(b'+' | b'-')) {
                self.i += 1;
            }
            let from = self.i;
            while matches!(self.s.get(self.i), Some(b'0'..=b'9')) {
                self.i += 1;
            }
            if self.i == from {
                return Err(self.fail("digits in the exponent"));
            }
        }
        let text = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| self.fail("not text"))?;
        let n: f64 = text.parse().map_err(|_| self.fail("not a number"))?;
        if n.is_finite() { Ok(Json::Num(n)) } else { Err(self.fail("a number too large")) }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self.s.get(self.i..self.i + 4).ok_or_else(|| self.fail("a short \\u escape"))?;
        let text = std::str::from_utf8(digits).map_err(|_| self.fail("not text"))?;
        let n = u32::from_str_radix(text, 16).map_err(|_| self.fail("a bad \\u escape"))?;
        self.i += 4;
        Ok(n)
    }

    fn string(&mut self) -> Result<String, String> {
        self.i += 1; // the opening quote
        let mut out = String::new();
        loop {
            // plain runs are copied whole, which also keeps multi-byte characters together
            let run = self.s[self.i..]
                .iter()
                .position(|b| matches!(b, b'"' | b'\\' | 0..=0x1f))
                .unwrap_or(self.s.len() - self.i);
            out.push_str(std::str::from_utf8(&self.s[self.i..self.i + run]).map_err(|_| self.fail("not UTF-8"))?);
            self.i += run;
            match self.s.get(self.i) {
                None => return Err(self.fail("a string that does not end")),
                Some(b'"') => {
                    self.i += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.i += 1;
                    let esc = *self.s.get(self.i).ok_or_else(|| self.fail("a string that does not end"))?;
                    self.i += 1;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let c = if (0xd800..0xdc00).contains(&hi) {
                                // the first half of a pair must be followed by the second
                                if !(self.eat(b'\\') && self.eat(b'u')) {
                                    return Err(self.fail("half of a surrogate pair"));
                                }
                                let lo = self.hex4()?;
                                if !(0xdc00..0xe000).contains(&lo) {
                                    return Err(self.fail("half of a surrogate pair"));
                                }
                                char::from_u32(0x10000 + ((hi - 0xd800) << 10) + (lo - 0xdc00))
                            } else {
                                char::from_u32(hi)
                            };
                            out.push(c.ok_or_else(|| self.fail("a lone surrogate"))?);
                        }
                        _ => return Err(self.fail("a bad escape")),
                    }
                }
                Some(_) => return Err(self.fail("a raw control character in a string")),
            }
        }
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding (an MCP image is carried as base64 text).
pub fn base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n =
            (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_reads_what_json_allows() {
        let v = Json::parse(r#" {"a": [1, -2.5, 3e2, true, false, null], "b": {"c": "d"}, "e": ""} "#).unwrap();
        assert_eq!(v.get("a").unwrap().as_array().unwrap().len(), 6);
        assert_eq!(v.get("a").unwrap().as_array().unwrap()[2].as_i64(), Some(300));
        assert_eq!(v.get("b").unwrap().get("c").unwrap().as_str(), Some("d"));
        assert_eq!(v.get("e").unwrap().as_str(), Some(""));
        assert_eq!(v.get("missing"), None);
        assert_eq!(Json::parse("[]").unwrap(), Json::Arr(vec![]));
        assert_eq!(Json::parse("{}").unwrap(), Json::Obj(vec![]));
    }

    #[test]
    fn strings_keep_their_escapes_and_their_characters() {
        let v = Json::parse(r#""tab\t quote\" slash\\ \/ nl\n é 😀 é 日本""#).unwrap();
        assert_eq!(v.as_str(), Some("tab\t quote\" slash\\ / nl\n é 😀 é 日本"));
        assert_eq!(Json::parse(r#""\u0000\u001f""#).unwrap().as_str(), Some("\u{0}\u{1f}"));
    }

    #[test]
    fn what_is_not_json_is_an_error() {
        for bad in [
            "",
            " ",
            "nul",
            "tru",
            "{",
            "[",
            "[1,]",
            "[1 2]",
            r#"{"a"}"#,
            r#"{"a":}"#,
            r#"{a:1}"#,
            "{'a':1}",
            r#""abc"#,
            "\"a\nb\"",
            r#""\x""#,
            r#""\u12""#,
            r#""\ud800""#,
            r#""\ud800A""#,
            r#""\udc00""#,
            "01",
            "1.",
            "1e",
            "-",
            "+1",
            ".5",
            "NaN",
            "Infinity",
            "1e999",
            "[1] 2",
            "{} {}",
            "\u{feff}{}",
        ] {
            assert!(Json::parse(bad).is_err(), "{bad:?} was accepted");
        }
    }

    #[test]
    fn it_does_not_follow_a_document_down_without_end() {
        let deep = "[".repeat(100_000);
        assert!(Json::parse(&deep).unwrap_err().contains("too deep"));
        let ok = format!("{}1{}", "[".repeat(60), "]".repeat(60));
        assert!(Json::parse(&ok).is_ok());
    }

    #[test]
    fn a_repeated_name_keeps_the_last_value() {
        let v = Json::parse(r#"{"a":1,"b":2,"a":3}"#).unwrap();
        assert_eq!(v.get("a").unwrap().as_i64(), Some(3));
        assert_eq!(v.write(), r#"{"b":2,"a":3}"#);
    }

    #[test]
    fn whole_numbers_are_told_from_the_rest() {
        assert_eq!(Json::Num(3.0).as_i64(), Some(3));
        assert_eq!(Json::Num(-7.0).as_i64(), Some(-7));
        assert_eq!(Json::Num(3.5).as_i64(), None);
        assert_eq!(Json::Num(1e30).as_i64(), None);
        assert_eq!(Json::Str("3".into()).as_i64(), None);
        assert_eq!(Json::Num(3.5).as_f64(), Some(3.5));
    }

    #[test]
    fn what_is_written_is_one_line_and_reads_back_the_same() {
        let v = obj([
            ("text", "a \"quote\"\nline\u{7}\u{2028} é 😀".into()),
            ("n", 42.into()),
            ("f", Json::Num(1.5)),
            ("list", Json::Arr(vec![Json::Null, true.into(), Json::Num(-3.0)])),
            ("o", obj([("k", "v".into())])),
        ]);
        let line = v.write();
        assert!(!line.contains('\n') && !line.contains('\r'), "{line}");
        assert_eq!(Json::parse(&line).unwrap(), v);
        assert_eq!(Json::Num(f64::NAN).write(), "null");
        assert_eq!(Json::Num(1234567.0).write(), "1234567");
    }

    #[test]
    fn base64_matches_the_standard_vectors() {
        // RFC 4648 section 10
        for (plain, coded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(plain.as_bytes()), coded);
        }
        assert_eq!(base64(&[0xff, 0xfe, 0xfd, 0x00]), "//79AA==");
    }
}
