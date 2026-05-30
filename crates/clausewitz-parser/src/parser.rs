use crate::lexer::{Lexer, Token, TokenKind};

/// Comparison operator between key and value
#[derive(Debug, Clone, PartialEq)]
pub enum Operator {
    Eq,
    Lt,
    Gt,
    Le,
    Ge,
    Ne,
}

/// A parsed value in Paradox Script
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Integer(i64),
    Float(f64),
    /// Percent literal (e.g. `width=100%%` → `Percent(100)`).
    /// Resolved against parent dimensions in `.gui` layout (4.1.bis).
    Percent(i32),
    String(String),
    /// A block containing key-value pairs and/or bare values (arrays)
    Block(Block),
}

/// A key-value entry with an operator
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub key: String,
    pub op: Operator,
    pub value: Value,
}

/// A block is a list of entries and/or bare values (for arrays like `{ 1 2 3 }`)
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub entries: Vec<Entry>,
    pub values: Vec<Value>,
}

impl Block {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Get first value for a key
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries.iter().find(|e| e.key == key).map(|e| &e.value)
    }

    /// Get all values for a key (for repeated keys like `focus = {...}`)
    pub fn get_all(&self, key: &str) -> Vec<&Value> {
        self.entries
            .iter()
            .filter(|e| e.key == key)
            .map(|e| &e.value)
            .collect()
    }

    /// Get as string
    pub fn get_string(&self, key: &str) -> Option<&str> {
        match self.get(key) {
            Some(Value::String(s)) => Some(s),
            _ => None,
        }
    }

    /// Get as integer
    pub fn get_int(&self, key: &str) -> Option<i64> {
        match self.get(key) {
            Some(Value::Integer(v)) => Some(*v),
            _ => None,
        }
    }

    /// Get as float
    pub fn get_float(&self, key: &str) -> Option<f64> {
        match self.get(key) {
            Some(Value::Float(v)) => Some(*v),
            Some(Value::Integer(v)) => Some(*v as f64),
            _ => None,
        }
    }

    /// Get as bool
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.get(key) {
            Some(Value::Bool(v)) => Some(*v),
            _ => None,
        }
    }

    /// Get as block
    pub fn get_block(&self, key: &str) -> Option<&Block> {
        match self.get(key) {
            Some(Value::Block(b)) => Some(b),
            _ => None,
        }
    }

    /// Get as percent literal (4.1.bis.1).
    /// Returns `None` if the key is missing or the value is not a percent.
    pub fn get_percent(&self, key: &str) -> Option<i32> {
        match self.get(key) {
            Some(Value::Percent(v)) => Some(*v),
            _ => None,
        }
    }
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&TokenKind> {
        self.tokens.get(self.pos).map(|t| &t.kind)
    }

    fn advance(&mut self) -> Option<&Token> {
        let t = self.tokens.get(self.pos)?;
        self.pos += 1;
        Some(t)
    }

    fn parse_block(&mut self) -> Block {
        let mut block = Block::new();
        loop {
            match self.peek() {
                None | Some(TokenKind::Close) => {
                    self.advance(); // consume Close if present
                    break;
                }
                _ => {}
            }

            // Try to parse as key = value
            let saved = self.pos;
            if let Some(key) = self.try_read_key() {
                if let Some(op) = self.try_read_operator() {
                    let value = self.parse_value();
                    block.entries.push(Entry { key, op, value });
                    continue;
                }
                // Not an assignment, rewind and treat as bare value
                self.pos = saved;
            }

            // Bare value (array element)
            let value = self.parse_value();
            block.values.push(value);
        }
        block
    }

    fn try_read_key(&mut self) -> Option<String> {
        match self.peek()? {
            TokenKind::Ident(_) | TokenKind::String(_) | TokenKind::Integer(_) => {
                let t = self.advance().unwrap();
                Some(match &t.kind {
                    TokenKind::Ident(s) | TokenKind::String(s) => s.clone(),
                    TokenKind::Integer(v) => v.to_string(),
                    _ => unreachable!(),
                })
            }
            _ => None,
        }
    }

    fn try_read_operator(&mut self) -> Option<Operator> {
        match self.peek()? {
            TokenKind::Eq => {
                self.advance();
                Some(Operator::Eq)
            }
            TokenKind::Lt => {
                self.advance();
                Some(Operator::Lt)
            }
            TokenKind::Gt => {
                self.advance();
                Some(Operator::Gt)
            }
            TokenKind::Le => {
                self.advance();
                Some(Operator::Le)
            }
            TokenKind::Ge => {
                self.advance();
                Some(Operator::Ge)
            }
            TokenKind::Ne => {
                self.advance();
                Some(Operator::Ne)
            }
            _ => None,
        }
    }

    fn parse_value(&mut self) -> Value {
        match self.peek() {
            Some(TokenKind::Open) => {
                self.advance(); // consume {
                Value::Block(self.parse_block())
            }
            Some(TokenKind::Bool(_)) => {
                let t = self.advance().unwrap();
                if let TokenKind::Bool(v) = t.kind {
                    Value::Bool(v)
                } else {
                    unreachable!()
                }
            }
            Some(TokenKind::Integer(_)) => {
                let t = self.advance().unwrap();
                if let TokenKind::Integer(v) = t.kind {
                    Value::Integer(v)
                } else {
                    unreachable!()
                }
            }
            Some(TokenKind::Float(_)) => {
                let t = self.advance().unwrap();
                if let TokenKind::Float(v) = t.kind {
                    Value::Float(v)
                } else {
                    unreachable!()
                }
            }
            Some(TokenKind::Percent(_)) => {
                let t = self.advance().unwrap();
                if let TokenKind::Percent(v) = t.kind {
                    Value::Percent(v)
                } else {
                    unreachable!()
                }
            }
            Some(TokenKind::String(_)) => {
                let t = self.advance().unwrap();
                if let TokenKind::String(ref s) = t.kind {
                    Value::String(s.clone())
                } else {
                    unreachable!()
                }
            }
            Some(TokenKind::Ident(_)) => {
                let t = self.advance().unwrap();
                let ident = if let TokenKind::Ident(ref s) = t.kind {
                    s.clone()
                } else {
                    unreachable!()
                };
                // Check for typed block: `rgb { ... }`, `HSV { ... }`, `hsv { ... }`, `list { ... }`
                // The ident becomes a tagged wrapper — we represent it as a Block with the tag as a single entry
                if matches!(self.peek(), Some(TokenKind::Open)) {
                    self.advance(); // consume {
                    let inner = self.parse_block();
                    let mut wrapper = Block::new();
                    wrapper.entries.push(Entry {
                        key: ident,
                        op: Operator::Eq,
                        value: Value::Block(inner),
                    });
                    return Value::Block(wrapper);
                }
                Value::String(ident)
            }
            _ => {
                self.advance();
                Value::String(String::new())
            }
        }
    }
}

/// Parse a Paradox Script string into a top-level Block
pub fn parse(input: &str) -> Block {
    let tokens = Lexer::new(input).tokenize();
    let mut parser = Parser::new(tokens);
    // Top-level is an implicit block (no braces)
    let mut block = Block::new();
    loop {
        if parser.peek().is_none() {
            break;
        }

        let saved = parser.pos;
        if let Some(key) = parser.try_read_key() {
            if let Some(op) = parser.try_read_operator() {
                let value = parser.parse_value();
                block.entries.push(Entry { key, op, value });
                continue;
            }
            parser.pos = saved;
        }

        let value = parser.parse_value();
        block.values.push(value);
    }
    block
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_kv() {
        let block = parse("id = 5\nname = \"hello\"");
        assert_eq!(block.get_int("id"), Some(5));
        assert_eq!(block.get_string("name"), Some("hello"));
    }

    #[test]
    fn test_nested_block() {
        let block = parse("state = { id = 5 manpower = 1000 }");
        let state = block.get_block("state").unwrap();
        assert_eq!(state.get_int("id"), Some(5));
        assert_eq!(state.get_int("manpower"), Some(1000));
    }

    #[test]
    fn test_array() {
        let block = parse("provinces = { 266 3351 3380 }");
        let provs = block.get_block("provinces").unwrap();
        assert_eq!(provs.values.len(), 3);
        assert_eq!(provs.values[0], Value::Integer(266));
    }

    #[test]
    fn test_repeated_keys() {
        let input = "focus = { id = a }\nfocus = { id = b }";
        let block = parse(input);
        let all = block.get_all("focus");
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_bool_values() {
        let block = parse("active = no\nreset_on_civilwar = yes");
        assert_eq!(block.get_bool("active"), Some(false));
        assert_eq!(block.get_bool("reset_on_civilwar"), Some(true));
    }

    #[test]
    fn test_comparison_operators() {
        let block = parse("date > 1936.10.4\nvalue <= 100");
        let e = &block.entries[0];
        assert_eq!(e.key, "date");
        assert_eq!(e.op, Operator::Gt);
        let e = &block.entries[1];
        assert_eq!(e.op, Operator::Le);
    }

    // 4.1.bis.1: percent values surface to the AST as `Value::Percent`.
    #[test]
    fn test_percent_as_value() {
        let block = parse("size = { width=100%% height=50%% }");
        let size = block.get_block("size").unwrap();
        assert_eq!(size.get_percent("width"), Some(100));
        assert_eq!(size.get_percent("height"), Some(50));
        // `get_int` on a percent must NOT silently coerce.
        assert_eq!(size.get_int("width"), None);
    }

    #[test]
    fn test_int_and_percent_coexist() {
        let block = parse("size = { width=1920 height=100%% }");
        let size = block.get_block("size").unwrap();
        assert_eq!(size.get_int("width"), Some(1920));
        assert_eq!(size.get_percent("height"), Some(100));
    }

    #[test]
    fn test_state_file() {
        let input = r#"state = {
    id = 5
    name = "STATE_5"
    manpower = 1238108
    state_category = town
    history = {
        owner = GER
        victory_points = { 6375 3 }
        buildings = {
            infrastructure = 3
        }
    }
    provinces = { 266 3351 3380 6375 }
    local_supplies = 12.0
}"#;
        let block = parse(input);
        let state = block.get_block("state").unwrap();
        assert_eq!(state.get_int("id"), Some(5));
        assert_eq!(state.get_string("name"), Some("STATE_5"));
        assert_eq!(state.get_int("manpower"), Some(1238108));
        assert_eq!(state.get_float("local_supplies"), Some(12.0));

        let history = state.get_block("history").unwrap();
        assert_eq!(history.get_string("owner"), Some("GER"));

        let provs = state.get_block("provinces").unwrap();
        assert_eq!(provs.values.len(), 4);
    }
}
