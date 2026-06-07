#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    /// Unquoted identifier: letters, digits, underscore, hyphen, dot, colon, @, /
    Ident(String),
    /// Quoted string literal
    String(String),
    /// Integer
    Integer(i64),
    /// Float
    Float(f64),
    /// yes/no
    Bool(bool),
    /// Percent literal — vanilla writes `100%%` (with double escape) for 100%.
    /// Single `%` after an integer is also recognised as percent (mod typo tolerance).
    Percent(i32),
    /// =
    Eq,
    /// <
    Lt,
    /// >
    Gt,
    /// <=
    Le,
    /// >=
    Ge,
    /// !=
    Ne,
    /// {
    Open,
    /// }
    Close,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: u32,
    pub col: u32,
}

pub struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    line: u32,
    col: u32,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        // Skip UTF-8 BOM
        let src = if src.as_bytes().starts_with(&[0xEF, 0xBB, 0xBF]) {
            &src[3..]
        } else {
            src
        };
        Self {
            src: src.as_bytes(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<u8> {
        let ch = self.src.get(self.pos).copied()?;
        self.pos += 1;
        if ch == b'\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(ch)
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(b' ' | b'\t' | b'\r' | b'\n') => {
                    self.advance();
                }
                Some(b'#') => {
                    while let Some(ch) = self.advance() {
                        if ch == b'\n' {
                            break;
                        }
                    }
                }
                _ => break,
            }
        }
    }

    fn read_quoted_string(&mut self) -> String {
        self.advance(); // skip opening quote
        let mut s = Vec::new();
        loop {
            match self.advance() {
                Some(b'"') | None => break,
                Some(b'\\') => match self.advance() {
                    Some(b'n') => s.push(b'\n'),
                    Some(b't') => s.push(b'\t'),
                    Some(b'\\') => s.push(b'\\'),
                    Some(b'"') => s.push(b'"'),
                    Some(ch) => {
                        s.push(b'\\');
                        s.push(ch);
                    }
                    None => break,
                },
                Some(ch) => s.push(ch),
            }
        }
        String::from_utf8_lossy(&s).into_owned()
    }

    fn is_ident_char(ch: u8) -> bool {
        // 4.1.bis.1: '%' is no longer an ident char — it now ends a number and
        // emits Token::Percent. vanilla writes `width=100%%` (with the lexer-escape
        // pair); without this fix the entire word `100%%` was an Ident, which
        // collapsed `topbar.gui::size` to (0,0).
        ch.is_ascii_alphanumeric()
            || matches!(
                ch,
                b'_' | b'-' | b'.' | b':' | b'@' | b'/' | b'\'' | b'[' | b']' | b'?'
            )
    }

    fn read_ident_or_number(&mut self) -> TokenKind {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if Self::is_ident_char(ch) {
                self.advance();
            } else {
                break;
            }
        }
        let word = &self.src[start..self.pos];
        let s = std::str::from_utf8(word).unwrap_or("");

        if s == "yes" {
            return TokenKind::Bool(true);
        }
        if s == "no" {
            return TokenKind::Bool(false);
        }

        // Try integer (4.1.bis.1: peek for trailing %% / % to recognise percent)
        if let Ok(v) = s.parse::<i64>() {
            if matches!(self.peek(), Some(b'%')) {
                self.advance();
                // optional second '%' (Paradox writes `100%%` for one percent token)
                if matches!(self.peek(), Some(b'%')) {
                    self.advance();
                }
                return TokenKind::Percent(v as i32);
            }
            return TokenKind::Integer(v);
        }
        // Try float (but not dates like 1936.1.1.12)
        if s.matches('.').count() == 1 {
            if let Ok(v) = s.parse::<f64>() {
                return TokenKind::Float(v);
            }
        }
        // Try negative number prefixed with -
        TokenKind::Ident(s.to_owned())
    }

    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            let line = self.line;
            let col = self.col;
            let kind = match self.peek() {
                None => break,
                Some(b'{') => {
                    self.advance();
                    TokenKind::Open
                }
                Some(b'}') => {
                    self.advance();
                    TokenKind::Close
                }
                Some(b'"') => TokenKind::String(self.read_quoted_string()),
                Some(b'=') => {
                    self.advance();
                    TokenKind::Eq
                }
                Some(b'<') => {
                    self.advance();
                    if self.peek() == Some(b'=') {
                        self.advance();
                        TokenKind::Le
                    } else {
                        TokenKind::Lt
                    }
                }
                Some(b'>') => {
                    self.advance();
                    if self.peek() == Some(b'=') {
                        self.advance();
                        TokenKind::Ge
                    } else {
                        TokenKind::Gt
                    }
                }
                Some(b'!') => {
                    self.advance();
                    if self.peek() == Some(b'=') {
                        self.advance();
                        TokenKind::Ne
                    } else {
                        TokenKind::Ident("!".to_owned())
                    }
                }
                Some(ch) if Self::is_ident_char(ch) || ch == b'+' => {
                    if ch == b'+' {
                        self.advance();
                    }
                    // handle negative numbers: if '-' followed by digit
                    match self.read_ident_or_number() {
                        TokenKind::Integer(v) if ch == b'+' => TokenKind::Integer(v),
                        TokenKind::Float(v) if ch == b'+' => TokenKind::Float(v),
                        TokenKind::Percent(v) if ch == b'+' => TokenKind::Percent(v),
                        TokenKind::Ident(s) if ch == b'+' => TokenKind::Ident(format!("+{s}")),
                        other => other,
                    }
                }
                Some(b'-') => {
                    // Could be negative number or ident starting with -
                    let next = self.src.get(self.pos + 1).copied();
                    if next.map_or(false, |c| c.is_ascii_digit()) {
                        self.advance(); // consume '-'
                        match self.read_ident_or_number() {
                            TokenKind::Integer(v) => TokenKind::Integer(-v),
                            TokenKind::Float(v) => TokenKind::Float(-v),
                            TokenKind::Percent(v) => TokenKind::Percent(-v),
                            other => other,
                        }
                    } else {
                        self.read_ident_or_number()
                    }
                }
                Some(b'%') => {
                    // Stray '%' not preceded by a digit. Skip it (Paradox sometimes
                    // emits `%` on its own after escapes; treat as whitespace).
                    self.advance();
                    continue;
                }
                Some(_) => {
                    self.advance();
                    continue;
                }
            };
            tokens.push(Token { kind, line, col });
        }
        tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tokens() {
        let input = "state = { id = 5 name = \"STATE_5\" }";
        let tokens = Lexer::new(input).tokenize();
        assert_eq!(tokens[0].kind, TokenKind::Ident("state".into()));
        assert_eq!(tokens[1].kind, TokenKind::Eq);
        assert_eq!(tokens[2].kind, TokenKind::Open);
        assert_eq!(tokens[3].kind, TokenKind::Ident("id".into()));
        assert_eq!(tokens[4].kind, TokenKind::Eq);
        assert_eq!(tokens[5].kind, TokenKind::Integer(5));
        assert_eq!(tokens[6].kind, TokenKind::Ident("name".into()));
        assert_eq!(tokens[7].kind, TokenKind::Eq);
        assert_eq!(tokens[8].kind, TokenKind::String("STATE_5".into()));
        assert_eq!(tokens[9].kind, TokenKind::Close);
    }

    #[test]
    fn test_bool_and_float() {
        let input = "active = no\nsupply_consumption = 0.06";
        let tokens = Lexer::new(input).tokenize();
        assert_eq!(tokens[2].kind, TokenKind::Bool(false));
        assert_eq!(tokens[5].kind, TokenKind::Float(0.06));
    }

    #[test]
    fn test_date_as_ident() {
        let input = "date > 1936.10.4";
        let tokens = Lexer::new(input).tokenize();
        assert_eq!(tokens[0].kind, TokenKind::Ident("date".into()));
        assert_eq!(tokens[1].kind, TokenKind::Gt);
        assert_eq!(tokens[2].kind, TokenKind::Ident("1936.10.4".into()));
    }

    #[test]
    fn test_comment_skip() {
        let input = "# comment\nkey = val";
        let tokens = Lexer::new(input).tokenize();
        assert_eq!(tokens[0].kind, TokenKind::Ident("key".into()));
    }

    #[test]
    fn test_bom() {
        let input = "\u{FEFF}key = 1";
        let tokens = Lexer::new(input).tokenize();
        assert_eq!(tokens[0].kind, TokenKind::Ident("key".into()));
    }

    #[test]
    fn test_operators() {
        let input = "a < 5 b >= 10 c != d";
        let tokens = Lexer::new(input).tokenize();
        assert_eq!(tokens[1].kind, TokenKind::Lt);
        assert_eq!(tokens[4].kind, TokenKind::Ge);
        assert_eq!(tokens[7].kind, TokenKind::Ne);
    }

    #[test]
    fn test_negative_number() {
        let input = "value = -100";
        let tokens = Lexer::new(input).tokenize();
        assert_eq!(tokens[2].kind, TokenKind::Integer(-100));
    }

    // 4.1.bis.1: percent literal handling.
    #[test]
    fn test_percent_double_escape() {
        // Vanilla `topbar.gui` writes `width=100%%` to encode a single percent.
        let input = "width=100%%";
        let tokens = Lexer::new(input).tokenize();
        assert_eq!(tokens[0].kind, TokenKind::Ident("width".into()));
        assert_eq!(tokens[1].kind, TokenKind::Eq);
        assert_eq!(tokens[2].kind, TokenKind::Percent(100));
    }

    #[test]
    fn test_percent_single() {
        // Some mods write `50%` once (typo). Tolerate it.
        let input = "size = 50%";
        let tokens = Lexer::new(input).tokenize();
        assert_eq!(tokens[2].kind, TokenKind::Percent(50));
    }

    #[test]
    fn test_percent_negative() {
        let input = "offset = -25%%";
        let tokens = Lexer::new(input).tokenize();
        assert_eq!(tokens[2].kind, TokenKind::Percent(-25));
    }

    #[test]
    fn test_pure_integer_still_works() {
        // After the lexer change, plain `1920` must still be an Integer.
        let input = "width = 1920";
        let tokens = Lexer::new(input).tokenize();
        assert_eq!(tokens[2].kind, TokenKind::Integer(1920));
    }

    #[test]
    fn test_plus_prefixed_value_advances() {
        let input = "offset = +25";
        let tokens = Lexer::new(input).tokenize();
        assert_eq!(tokens[2].kind, TokenKind::Integer(25));
    }

    #[test]
    fn test_percent_inside_size_block() {
        // Real-world scenario from `topbar.gui:5`:
        let input = "size = { width=100%% height=100%% }";
        let tokens = Lexer::new(input).tokenize();
        // size = { width = 100%% height = 100%% }
        assert!(tokens
            .iter()
            .any(|t| matches!(&t.kind, TokenKind::Percent(100))));
    }
}
