//! Parser for Paradox Lua defines files (e.g. `00_defines.lua`).
//!
//! Format: `NDefines = { NSection = { KEY = value, ... }, ... }`
//! Values can be: integers, floats, strings, arrays of numbers.
//! Comments use `--`.

use std::collections::HashMap;

/// A single define value
#[derive(Debug, Clone, PartialEq)]
pub enum DefineValue {
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<f64>),
    Bool(bool),
}

impl DefineValue {
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(v) => Some(*v),
            Self::Float(v) => Some(*v as i64),
            _ => None,
        }
    }
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(v) => Some(*v),
            Self::Int(v) => Some(*v as f64),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_array(&self) -> Option<&[f64]> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }
}

/// Parsed defines: section_name -> (key -> value)
#[derive(Debug, Clone)]
pub struct Defines {
    pub sections: HashMap<String, HashMap<String, DefineValue>>,
}

impl Defines {
    /// Get a value by section and key: `defines.get("NGame", "START_DATE")`
    pub fn get(&self, section: &str, key: &str) -> Option<&DefineValue> {
        self.sections.get(section)?.get(key)
    }

    pub fn get_int(&self, section: &str, key: &str) -> Option<i64> {
        self.get(section, key)?.as_int()
    }

    pub fn get_float(&self, section: &str, key: &str) -> Option<f64> {
        self.get(section, key)?.as_float()
    }

    pub fn get_str(&self, section: &str, key: &str) -> Option<&str> {
        self.get(section, key)?.as_str()
    }

    /// Merge another defines into this one (for DLC overrides)
    pub fn merge(&mut self, other: Defines) {
        for (section, entries) in other.sections {
            self.sections.entry(section).or_default().extend(entries);
        }
    }
}

/// Parse a Lua defines file into a `Defines` struct.
pub fn parse_defines(input: &str) -> Defines {
    let mut defines = Defines {
        sections: HashMap::new(),
    };
    let cleaned = strip_comments(input);
    parse_top_level(&cleaned, &mut defines);
    defines
}

fn strip_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for line in input.lines() {
        if let Some(idx) = line.find("--") {
            out.push_str(&line[..idx]);
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

fn parse_top_level(input: &str, defines: &mut Defines) {
    // Find the outer table: `NDefines = {` or `NDefines_Graphics = {`
    // Then find each section: `NGame = {`
    let mut pos = 0;

    // Skip to first `{` (the outer NDefines table)
    while pos < input.len() {
        if input.as_bytes()[pos] == b'{' {
            pos += 1;
            break;
        }
        pos += 1;
    }

    // Now parse sections inside
    let inner = &input[pos..];
    parse_sections(inner, defines);
}

fn parse_sections(input: &str, defines: &mut Defines) {
    let bytes = input.as_bytes();
    let mut pos = 0;

    loop {
        skip_ws(bytes, &mut pos);
        if pos >= bytes.len() {
            break;
        }
        if bytes[pos] == b'}' {
            break;
        } // end of outer table

        // Read section name (e.g. "NGame")
        let name = read_ident(bytes, &mut pos);
        if name.is_empty() {
            pos += 1;
            continue;
        }

        skip_ws(bytes, &mut pos);
        // Expect `=`
        if pos < bytes.len() && bytes[pos] == b'=' {
            pos += 1;
        }
        skip_ws(bytes, &mut pos);
        // Expect `{`
        if pos < bytes.len() && bytes[pos] == b'{' {
            pos += 1;
            let entries = parse_entries(bytes, &mut pos);
            defines.sections.insert(name, entries);
        }
    }
}

fn parse_entries(bytes: &[u8], pos: &mut usize) -> HashMap<String, DefineValue> {
    let mut map = HashMap::new();

    loop {
        skip_ws(bytes, pos);
        if *pos >= bytes.len() {
            break;
        }
        if bytes[*pos] == b'}' {
            *pos += 1;
            break;
        }

        let key = read_ident(bytes, pos);
        if key.is_empty() {
            *pos += 1;
            continue;
        }

        skip_ws(bytes, pos);
        if *pos < bytes.len() && bytes[*pos] == b'=' {
            *pos += 1;
            skip_ws(bytes, pos);

            // Check if value is a nested section (subsection like `NGame = { ... }`)
            if *pos < bytes.len() && bytes[*pos] == b'{' {
                // Could be an array `{ 1, 2, 3 }` or a nested section
                *pos += 1;
                skip_ws(bytes, pos);

                // Peek: if next non-ws is an ident followed by `=`, it's a nested section
                let saved = *pos;
                let peek_ident = read_ident(bytes, pos);
                skip_ws(bytes, pos);
                let is_section =
                    !peek_ident.is_empty() && *pos < bytes.len() && bytes[*pos] == b'=';
                *pos = saved;

                if is_section {
                    // It's a nested section — recurse (treat as sub-defines)
                    let entries = parse_entries(bytes, pos);
                    // Flatten: store each entry with section prefix already handled by caller
                    // Actually this IS the section content
                    map.extend(entries);
                } else {
                    // It's an array
                    let arr = parse_array(bytes, pos);
                    map.insert(key, DefineValue::Array(arr));
                }
            } else {
                // Scalar value
                let value = parse_value(bytes, pos);
                map.insert(key, value);
            }
        }

        // Skip trailing comma
        skip_ws(bytes, pos);
        if *pos < bytes.len() && bytes[*pos] == b',' {
            *pos += 1;
        }
    }
    map
}

fn parse_array(bytes: &[u8], pos: &mut usize) -> Vec<f64> {
    let mut arr = Vec::new();
    loop {
        skip_ws(bytes, pos);
        if *pos >= bytes.len() {
            break;
        }
        if bytes[*pos] == b'}' {
            *pos += 1;
            break;
        }
        if bytes[*pos] == b',' {
            *pos += 1;
            continue;
        }

        let start = *pos;
        while *pos < bytes.len()
            && !matches!(bytes[*pos], b',' | b'}' | b' ' | b'\t' | b'\n' | b'\r')
        {
            *pos += 1;
        }
        let s = std::str::from_utf8(&bytes[start..*pos]).unwrap_or("");
        if let Ok(v) = s.parse::<f64>() {
            arr.push(v);
        }
    }
    arr
}

fn parse_value(bytes: &[u8], pos: &mut usize) -> DefineValue {
    skip_ws(bytes, pos);
    if *pos >= bytes.len() {
        return DefineValue::Int(0);
    }

    // String
    if bytes[*pos] == b'"' {
        *pos += 1;
        let start = *pos;
        while *pos < bytes.len() && bytes[*pos] != b'"' {
            *pos += 1;
        }
        let s = std::str::from_utf8(&bytes[start..*pos])
            .unwrap_or("")
            .to_owned();
        if *pos < bytes.len() {
            *pos += 1;
        } // skip closing quote
        return DefineValue::String(s);
    }

    // Number or bool
    let start = *pos;
    while *pos < bytes.len() && !matches!(bytes[*pos], b',' | b'}' | b' ' | b'\t' | b'\n' | b'\r') {
        *pos += 1;
    }
    let s = std::str::from_utf8(&bytes[start..*pos])
        .unwrap_or("")
        .trim();

    if s == "true" {
        return DefineValue::Bool(true);
    }
    if s == "false" {
        return DefineValue::Bool(false);
    }
    if let Ok(v) = s.parse::<i64>() {
        return DefineValue::Int(v);
    }
    if let Ok(v) = s.parse::<f64>() {
        return DefineValue::Float(v);
    }
    DefineValue::String(s.to_owned())
}

fn skip_ws(bytes: &[u8], pos: &mut usize) {
    while *pos < bytes.len() && matches!(bytes[*pos], b' ' | b'\t' | b'\n' | b'\r') {
        *pos += 1;
    }
}

fn read_ident(bytes: &[u8], pos: &mut usize) -> String {
    let start = *pos;
    while *pos < bytes.len() && (bytes[*pos].is_ascii_alphanumeric() || bytes[*pos] == b'_') {
        *pos += 1;
    }
    std::str::from_utf8(&bytes[start..*pos])
        .unwrap_or("")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_defines() {
        let input = r#"
NDefines = {
NGame = {
    START_DATE = "1936.1.1.12",
    END_DATE = "1949.1.1.1",
    MAP_SCALE_PIXEL_TO_KM = 7.114, -- comment
    SAVE_VERSION = 31,
    GAME_SPEED_SECONDS = { 2.0, 0.5, 0.2, 0.1, 0.0 },
},
}
"#;
        let d = parse_defines(input);
        assert_eq!(d.get_str("NGame", "START_DATE"), Some("1936.1.1.12"));
        assert_eq!(d.get_int("NGame", "SAVE_VERSION"), Some(31));
        assert_eq!(d.get_float("NGame", "MAP_SCALE_PIXEL_TO_KM"), Some(7.114));
        let arr = d
            .get("NGame", "GAME_SPEED_SECONDS")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(arr.len(), 5);
        assert_eq!(arr[0], 2.0);
        assert_eq!(arr[4], 0.0);
    }

    #[test]
    fn test_multiple_sections() {
        let input = r#"
NDefines = {
NGame = {
    START_DATE = "1936.1.1.12",
},
NDiplomacy = {
    BASE_SURRENDER_LEVEL = 1.0,
    MAX_TRUST_VALUE = 100,
},
}
"#;
        let d = parse_defines(input);
        assert_eq!(d.get_str("NGame", "START_DATE"), Some("1936.1.1.12"));
        assert_eq!(d.get_float("NDiplomacy", "BASE_SURRENDER_LEVEL"), Some(1.0));
        assert_eq!(d.get_int("NDiplomacy", "MAX_TRUST_VALUE"), Some(100));
    }

    #[test]
    fn test_merge() {
        let base = r#"NDefines = { NGame = { X = 1, Y = 2, }, }"#;
        let override_str = r#"NDefines_Graphics = { NGame = { X = 99, }, }"#;
        let mut d = parse_defines(base);
        let d2 = parse_defines(override_str);
        d.merge(d2);
        assert_eq!(d.get_int("NGame", "X"), Some(99)); // overridden
        assert_eq!(d.get_int("NGame", "Y"), Some(2)); // preserved
    }

    #[test]
    fn test_negative_values() {
        let input = r#"NDefines = { NTest = { A = -100, B = -0.5, }, }"#;
        let d = parse_defines(input);
        assert_eq!(d.get_int("NTest", "A"), Some(-100));
        assert_eq!(d.get_float("NTest", "B"), Some(-0.5));
    }
}
