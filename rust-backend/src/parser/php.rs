//! A PHP-array value model: the compose parsers work on what `Symfony\Yaml::parse` returns (ordered
//! arrays with int or string keys), and the golden snapshots are dumps of such arrays.

/// PHP array key. Decimal integer strings become int keys, as PHP does on insert.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    Int(i64),
    Str(String),
}

impl Key {
    /// PHP's array key cast: `"7"` -> `7`, `"07"` / `"-0"` / `"1.5"` stay strings.
    pub fn parse(s: &str) -> Key {
        let canonical = s == "0"
            || (!s.is_empty()
                && !s.starts_with('0')
                && !s.starts_with("-0")
                && s != "-"
                && s.strip_prefix('-')
                    .unwrap_or(s)
                    .bytes()
                    .all(|b| b.is_ascii_digit()));
        match canonical.then(|| s.parse::<i64>().ok()).flatten() {
            Some(i) => Key::Int(i),
            None => Key::Str(s.to_string()),
        }
    }

    pub fn as_string(&self) -> String {
        match self {
            Key::Int(i) => i.to_string(),
            Key::Str(s) => s.clone(),
        }
    }
}

impl From<&str> for Key {
    fn from(s: &str) -> Self {
        Key::parse(s)
    }
}

impl From<String> for Key {
    fn from(s: String) -> Self {
        Key::parse(&s)
    }
}

impl From<i64> for Key {
    fn from(i: i64) -> Self {
        Key::Int(i)
    }
}

/// A PHP value as produced by Symfony YAML / used in the snapshots.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Array(Array),
}

/// Ordered PHP array.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Array {
    entries: Vec<(Key, Value)>,
    next_index: i64,
}

impl Array {
    pub fn new() -> Self {
        Self::default()
    }

    /// `$array[$key] = $value` (replaces in place when the key exists, like PHP).
    pub fn set(&mut self, key: impl Into<Key>, value: Value) {
        let key = key.into();
        if let Key::Int(i) = key {
            if i >= self.next_index {
                self.next_index = i + 1;
            }
        }
        match self.entries.iter_mut().find(|(k, _)| *k == key) {
            Some(entry) => entry.1 = value,
            None => self.entries.push((key, value)),
        }
    }

    /// `$array[] = $value`.
    pub fn push(&mut self, value: Value) {
        let key = Key::Int(self.next_index);
        self.set(key, value);
    }

    pub fn get(&self, key: impl Into<Key>) -> Option<&Value> {
        let key = key.into();
        self.entries.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
    }

    pub fn get_mut(&mut self, key: impl Into<Key>) -> Option<&mut Value> {
        let key = key.into();
        self.entries
            .iter_mut()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v)
    }

    pub fn remove(&mut self, key: impl Into<Key>) -> Option<Value> {
        let key = key.into();
        let pos = self.entries.iter().position(|(k, _)| *k == key)?;
        Some(self.entries.remove(pos).1)
    }

    pub fn contains_key(&self, key: impl Into<Key>) -> bool {
        self.get(key).is_some()
    }

    pub fn iter(&self) -> impl Iterator<Item = &(Key, Value)> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Symfony `Inline::isHash`: not a list with keys 0..n-1 in order.
    pub fn is_hash(&self) -> bool {
        self.entries
            .iter()
            .enumerate()
            .any(|(i, (k, _))| *k != Key::Int(i as i64))
    }

    pub fn from_list(values: impl IntoIterator<Item = Value>) -> Self {
        let mut a = Array::new();
        for v in values {
            a.push(v);
        }
        a
    }
}

impl FromIterator<(Key, Value)> for Array {
    fn from_iter<T: IntoIterator<Item = (Key, Value)>>(iter: T) -> Self {
        let mut a = Array::new();
        for (k, v) in iter {
            a.set(k, v);
        }
        a
    }
}

impl Value {
    pub fn str(s: impl Into<String>) -> Value {
        Value::Str(s.into())
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Array> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    /// `data_get($value, 'a.b.c')` for arrays (no wildcards).
    pub fn data_get(&self, path: &str) -> Option<&Value> {
        path.split('.')
            .try_fold(self, |v, part| v.as_array()?.get(part))
    }

    /// PHP string cast of a scalar (`(string) $value`).
    pub fn to_php_string(&self) -> String {
        match self {
            Value::Null => String::new(),
            Value::Bool(true) => "1".into(),
            Value::Bool(false) => String::new(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => php_float_to_string(*f),
            Value::Str(s) => s.clone(),
            Value::Array(_) => "Array".into(),
        }
    }
}

/// PHP `(string) $float` for the values compose files carry (shortest round-trip form).
pub fn php_float_to_string(f: f64) -> String {
    if f.is_nan() {
        return "NAN".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "INF".into() } else { "-INF".into() };
    }
    if f == f.trunc() && f.abs() < 1e15 {
        return format!("{}", f as i64);
    }
    let s = format!("{f}");
    if let Some((m, e)) = s.split_once('e') {
        // Rust "1e20" -> PHP "1.0E+20"
        let m = if m.contains('.') {
            m.to_string()
        } else {
            format!("{m}.0")
        };
        let (sign, digits) = e.strip_prefix('-').map_or(("+", e), |d| ("-", d));
        return format!("{m}E{sign}{digits}");
    }
    s
}

/// PHP `is_numeric()` for strings: optional leading/trailing whitespace, sign, digits with an
/// optional fraction and exponent.
pub fn is_numeric_str(s: &str) -> bool {
    let ws = |c: char| matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0B' | '\x0C');
    let t = s.trim_start_matches(ws).trim_end_matches(ws);
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let int_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut digits = i - int_start;
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let frac_start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        digits += i - frac_start;
    }
    if digits == 0 {
        return false;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let save = i;
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let exp_start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == exp_start {
            i = save;
        }
    }
    i == b.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_cast_like_php() {
        assert_eq!(Key::parse("7"), Key::Int(7));
        assert_eq!(Key::parse("-3"), Key::Int(-3));
        assert_eq!(Key::parse("07"), Key::Str("07".into()));
        assert_eq!(Key::parse("1.5"), Key::Str("1.5".into()));
        assert_eq!(Key::parse("api"), Key::Str("api".into()));
    }

    #[test]
    fn is_hash_and_push() {
        let mut a = Array::new();
        a.push(Value::Int(1));
        a.push(Value::Int(2));
        assert!(!a.is_hash());
        a.set("x", Value::Null);
        assert!(a.is_hash());
    }

    #[test]
    fn numeric_strings() {
        for s in ["1", "-1.5", " 12", "12 ", "1e5", ".5", "5."] {
            assert!(is_numeric_str(s), "{s}");
        }
        for s in ["", "abc", "1e", "1.2.3", "0x1A", "."] {
            assert!(!is_numeric_str(s), "{s}");
        }
    }
}
