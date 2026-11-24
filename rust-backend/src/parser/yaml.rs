//! YAML in and out with Symfony's semantics: `parse` loads a compose file into PHP arrays
//! (merge keys and aliases resolved), `dump` is a port of `Symfony\Component\Yaml\Dumper::dump` with
//! `Yaml::DUMP_MULTI_LINE_LITERAL_BLOCK` (the only flag the parsers and the golden test use).

use std::sync::OnceLock;

use regex::Regex;
use yaml_rust2::{Yaml, YamlLoader};

use super::php::{is_numeric_str, php_float_to_string, Array, Key, Value};

/// `parseDockerComposeYaml`: BOM stripped; on failure retry with anchors joined to their keys.
pub fn parse_compose(compose: &str) -> Result<Value, String> {
    let compose = compose.strip_prefix('\u{FEFF}').unwrap_or(compose);
    match parse(compose) {
        Ok(v) => Ok(v),
        Err(first) => {
            let joined = join_compose_anchors_to_their_keys(compose);
            if joined == compose {
                return Err(first);
            }
            parse(&joined).map_err(|_| first)
        }
    }
}

/// Parse one YAML document into a PHP value (empty document -> null).
pub fn parse(source: &str) -> Result<Value, String> {
    let docs = YamlLoader::load_from_str(source).map_err(|e| e.to_string())?;
    Ok(docs.first().map_or(Value::Null, convert))
}

fn convert(y: &Yaml) -> Value {
    match y {
        Yaml::Null | Yaml::BadValue | Yaml::Alias(_) => Value::Null,
        Yaml::Boolean(b) => Value::Bool(*b),
        Yaml::Integer(i) => Value::Int(*i),
        Yaml::Real(r) => yaml_float(r),
        Yaml::String(s) => Value::Str(s.clone()),
        Yaml::Array(items) => Value::Array(Array::from_list(items.iter().map(convert))),
        Yaml::Hash(map) => {
            let mut out = Array::new();
            for (k, v) in map {
                if matches!(k, Yaml::String(s) if s == "<<") {
                    // Symfony: `$data += $merged` — merged keys only fill keys not set yet.
                    let sources: Vec<&Yaml> = match v {
                        Yaml::Array(list) => list.iter().collect(),
                        other => vec![other],
                    };
                    for source in sources {
                        if let Value::Array(merged) = convert(source) {
                            for (mk, mv) in merged.iter() {
                                if !out.contains_key(mk.clone()) {
                                    out.set(mk.clone(), mv.clone());
                                }
                            }
                        }
                    }
                    continue;
                }
                out.set(yaml_key(k), convert(v));
            }
            Value::Array(out)
        }
    }
}

/// Symfony floats: digits (with fraction/exponent) or `.inf`/`.nan`; Rust's `f64` parser also accepts
/// words like `infinity`, which Symfony keeps as strings.
fn yaml_float(r: &str) -> Value {
    let lower = r.to_lowercase();
    let special = match lower.trim_start_matches(['+', '-']) {
        ".inf" | ".nan" => true,
        word if word.chars().any(|c| c.is_ascii_alphabetic() && c != 'e') => {
            return Value::Str(r.to_string())
        }
        _ => false,
    };
    let parsed = if special {
        match lower.as_str() {
            ".nan" => Some(f64::NAN),
            "-.inf" => Some(f64::NEG_INFINITY),
            _ => Some(f64::INFINITY),
        }
    } else {
        r.parse::<f64>().ok()
    };
    parsed.map_or_else(|| Value::Str(r.to_string()), Value::Float)
}

fn yaml_key(k: &Yaml) -> Key {
    match k {
        Yaml::Integer(i) => Key::Int(*i),
        Yaml::String(s) => Key::parse(s),
        Yaml::Boolean(true) => Key::Int(1),
        Yaml::Boolean(false) => Key::Int(0),
        Yaml::Real(r) => Key::parse(r),
        _ => Key::Str(String::new()),
    }
}

/// `joinComposeAnchorsToTheirKeys`: "key:\n  &name" -> "key: &name" when the anchor is indented deeper.
fn join_compose_anchors_to_their_keys(compose: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            r"(?m)^([ \t]*)((?:- )?[^\s#][^\n]*?:)[ \t]*(?:#[^\n]*)?\r?\n([ \t]+)(&[^\s#]+)[ \t]*(?:#[^\n]*)?\r?$",
        )
        .expect("valid regex")
    });
    re.replace_all(compose, |c: &regex::Captures| {
        if c[3].len() > c[1].len() {
            format!("{}{} {}", &c[1], &c[2], &c[4])
        } else {
            c[0].to_string()
        }
    })
    .into_owned()
}

// ---------------------------------------------------------------------------------------------
// Dumper
// ---------------------------------------------------------------------------------------------

const INDENTATION: usize = 2;

/// `Yaml::dump($value, $inline, 2, Yaml::DUMP_MULTI_LINE_LITERAL_BLOCK)`.
pub fn dump(input: &Value, inline: i32) -> String {
    do_dump(input, inline, 0)
}

fn is_nonempty_array(v: &Value) -> bool {
    matches!(v, Value::Array(a) if !a.is_empty())
}

fn do_dump(input: &Value, inline: i32, indent: usize) -> String {
    let prefix = " ".repeat(indent);
    let array = match input {
        Value::Array(a) if inline > 0 && !a.is_empty() => a,
        other => return format!("{prefix}{}", inline_dump(other)),
    };
    let as_map = array.is_hash();
    let mut output = String::new();
    for (key, value) in array.iter() {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        let head = if as_map {
            format!("{}:", inline_dump_key(key))
        } else {
            "-".to_string()
        };
        if let Value::Str(s) = value {
            if can_be_dumped_as_literal_block(s) {
                output.push_str(&format!(
                    "{prefix}{head} |{}{}",
                    block_indentation_indicator(s),
                    block_chomping_indicator(s)
                ));
                output.push_str(&dump_block_scalar_lines(s, &prefix));
                continue;
            }
        }
        let will_be_inlined = inline - 1 <= 0 || !is_nonempty_array(value);
        output.push_str(&prefix);
        output.push_str(&head);
        output.push(if will_be_inlined { ' ' } else { '\n' });
        output.push_str(&do_dump(
            value,
            inline - 1,
            if will_be_inlined {
                0
            } else {
                indent + INDENTATION
            },
        ));
        if will_be_inlined {
            output.push('\n');
        }
    }
    output
}

fn block_chomping_indicator(value: &str) -> &'static str {
    if value == "\n" || value.ends_with("\n\n") {
        "+"
    } else if value.ends_with('\n') {
        ""
    } else {
        "-"
    }
}

fn dump_block_scalar_lines(value: &str, prefix: &str) -> String {
    let mut out = String::new();
    for row in value.split('\n') {
        if row.is_empty() {
            out.push('\n');
        } else {
            out.push_str(&format!("\n{prefix}{}{row}", " ".repeat(INDENTATION)));
        }
    }
    out
}

fn block_indentation_indicator(value: &str) -> String {
    match value.split('\n').find(|l| !l.is_empty()) {
        Some(line) if line.starts_with(' ') => INDENTATION.to_string(),
        _ => String::new(),
    }
}

fn can_be_dumped_as_literal_block(value: &str) -> bool {
    value.contains('\n')
        && !value.chars().any(|c| {
            matches!(c, '\x00'..='\x08' | '\x0b'..='\x1f' | '\x7f' | '\u{85}' | '\u{2028}' | '\u{2029}')
        })
}

fn inline_dump_key(key: &Key) -> String {
    match key {
        Key::Int(i) => i.to_string(),
        Key::Str(s) => inline_dump_str(s),
    }
}

/// `Inline::dump` (no flags).
pub fn inline_dump(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(true) => "true".into(),
        Value::Bool(false) => "false".into(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => {
            if f.is_nan() {
                return ".NaN".into();
            }
            let mut repr = php_float_to_string(*f);
            if f.is_infinite() {
                repr = repr.replace("INF", ".Inf");
            } else if f.floor() == *f && !repr.contains('E') && !repr.contains('.') {
                repr.push_str(".0");
            }
            repr
        }
        Value::Str(s) => inline_dump_str(s),
        Value::Array(a) => {
            if a.is_empty() {
                return "{}".into();
            }
            if !a.is_hash() {
                let items: Vec<String> = a.iter().map(|(_, v)| inline_dump(v)).collect();
                return format!("[{}]", items.join(", "));
            }
            let items: Vec<String> = a
                .iter()
                .map(|(k, v)| format!("{}: {}", inline_dump_key(k), inline_dump(v)))
                .collect();
            format!("{{ {} }}", items.join(", "))
        }
    }
}

fn inline_dump_str(s: &str) -> String {
    if is_numeric_str(s) && !s.contains(['\x0C', '\n', '\r', '\t', '\x0B']) {
        return format!("'{s}'");
    }
    if s.is_empty() {
        return "''".into();
    }
    if is_binary_string(s) {
        use base64::Engine;
        return format!(
            "!!binary {}",
            base64::engine::general_purpose::STANDARD.encode(s.as_bytes())
        );
    }
    if requires_double_quoting(s) {
        return escape_with_double_quotes(s);
    }
    if requires_single_quoting(s) {
        let single = format!("'{}'", s.replace('\'', "''"));
        if !s.contains('\'') {
            return single;
        }
        let double = escape_with_double_quotes(s);
        return if double.len() < single.len() {
            double
        } else {
            single
        };
    }
    if looks_like_number_or_timestamp(s) {
        return format!("'{s}'");
    }
    s.to_string()
}

fn looks_like_number_or_timestamp(s: &str) -> bool {
    static RES: OnceLock<[Regex; 4]> = OnceLock::new();
    let res = RES.get_or_init(|| {
        [
            Regex::new(r"^[+-]?[0-9][0-9_]*(\.[0-9_]+)?$").unwrap(),
            Regex::new(r"^[+-]?0o[0-7_]+$").unwrap(),
            Regex::new(r"^0x[0-9a-fA-F_]+$").unwrap(),
            Regex::new(
                r"(?x)^
                [0-9][0-9][0-9][0-9]
                -[0-9][0-9]?
                -[0-9][0-9]?
                (?:(?:[Tt]|[\ \t]+)
                [0-9][0-9]?
                :[0-9][0-9]
                :[0-9][0-9]
                (?:\.[0-9]*)?
                (?:[\ \t]*(?:Z|[-+][0-9][0-9]?(?::[0-9][0-9])?))?)?
                $",
            )
            .unwrap(),
        ]
    });
    res.iter().any(|r| r.is_match(s))
}

/// `Inline::isBinaryString` (strings are valid UTF-8 here, so only control bytes count).
fn is_binary_string(s: &str) -> bool {
    s.bytes()
        .any(|b| matches!(b, 0x01..=0x06 | 0x0E..=0x1A | 0x1C..=0x1F))
}

fn requires_double_quoting(s: &str) -> bool {
    s.chars().any(|c| {
        matches!(
            c,
            '\x00'..='\x1f' | '\x7f' | '\u{85}' | '\u{a0}' | '\u{2028}' | '\u{2029}'
        )
    })
}

fn requires_single_quoting(s: &str) -> bool {
    let lower = s.to_lowercase();
    if [
        "null", "~", "true", "false", "y", "n", "yes", "no", "on", "off", ".inf", ".nan", "...",
    ]
    .contains(&lower.as_str())
    {
        return true;
    }
    if s.chars().any(|c| {
        c.is_whitespace()
            || matches!(
                c,
                '\'' | '"' | ':' | '{' | '}' | '[' | ']' | ',' | '&' | '*' | '#' | '?'
            )
    }) {
        return true;
    }
    s.chars().next().is_some_and(|c| {
        matches!(
            c,
            '-' | '?' | '|' | '<' | '>' | '=' | '!' | '%' | '@' | '`' | '\u{FEFF}'
        ) || is_space_separator(c)
    })
}

/// Unicode category Zs.
fn is_space_separator(c: char) -> bool {
    matches!(
        c,
        ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
    )
}

fn escape_with_double_quotes(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\x00' => out.push_str("\\0"),
            '\x07' => out.push_str("\\a"),
            '\x08' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\x0B' => out.push_str("\\v"),
            '\x0C' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            '\x1B' => out.push_str("\\e"),
            '\u{85}' => out.push_str("\\N"),
            '\u{a0}' => out.push_str("\\_"),
            '\u{2028}' => out.push_str("\\L"),
            '\u{2029}' => out.push_str("\\P"),
            c if (c as u32) < 0x20 || c == '\x7f' => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalars_quote_like_symfony() {
        assert_eq!(inline_dump_str("http://a.b/api"), "'http://a.b/api'");
        assert_eq!(inline_dump_str("3000"), "'3000'");
        assert_eq!(inline_dump_str("plain"), "plain");
        assert_eq!(inline_dump_str("yes"), "'yes'");
        // Symfony takes the shorter of the two quotings when the value contains a quote.
        assert_eq!(inline_dump_str("it's: x"), "\"it's: x\"");
        assert_eq!(inline_dump_str("a\"b'c"), "'a\"b''c'"); // equal length: single wins
        assert_eq!(inline_dump_str("/data/x"), "/data/x");
        assert_eq!(inline_dump_str("-x"), "'-x'");
    }

    #[test]
    fn merge_keys_fill_missing_only() {
        let v = parse("base: &b\n  a: 1\n  b: 2\nx:\n  <<: *b\n  b: 3\n").unwrap();
        let x = v.data_get("x").unwrap().as_array().unwrap();
        assert_eq!(x.get("a"), Some(&Value::Int(1)));
        assert_eq!(x.get("b"), Some(&Value::Int(3)));
    }
}
