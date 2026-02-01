// ============================================================================
// KORE Bootstrap Compiler - Lexer (Rust)
// ============================================================================
// Hand-translated from lexer.kr for Project Ouroboros bootstrap
// This is the self-hosted KORE lexer running as native Rust code.
// ============================================================================

use std::fmt;

// =============================================================================
// Token Types
// =============================================================================

/// All possible token types in KORE
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    
    // Identifiers and Keywords
    Ident(String),
    Keyword(String),
    
    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    EqEq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    And,
    Or,
    Not,
    Arrow,      // ->
    FatArrow,   // =>
    Dot,
    DotDot,     // ..
    Colon,
    ColonColon, // ::
    Comma,
    Semicolon,
    Pipe,       // |
    Ampersand,  // &
    
    // Brackets
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    LAngle,     // <
    RAngle,     // >
    
    // Whitespace (significant in KORE!)
    Newline,
    Indent,
    Dedent,
    
    // Special
    Eof,
    Error(String),
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Int(n) => write!(f, "Int({})", n),
            TokenKind::Float(n) => write!(f, "Float({})", n),
            TokenKind::String(s) => write!(f, "String(\"{}\")", s),
            TokenKind::Bool(b) => write!(f, "Bool({})", b),
            TokenKind::Ident(s) => write!(f, "Ident({})", s),
            TokenKind::Keyword(s) => write!(f, "Keyword({})", s),
            TokenKind::Error(s) => write!(f, "Error({})", s),
            _ => write!(f, "{:?}", self),
        }
    }
}

// =============================================================================
// Token
// =============================================================================

/// A token with its kind and source location
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub column: usize,
    pub lexeme: String,
}

impl Token {
    pub fn new(kind: TokenKind, line: usize, column: usize, lexeme: String) -> Token {
        Token { kind, line, column, lexeme }
    }
    
    pub fn is_keyword(&self, kw: &str) -> bool {
        let keywords = [
            "fn", "let", "var", "if", "else", "while", "for", "in",
            "return", "match", "struct", "enum", "impl", "use", "pub",
            "async", "await", "spawn", "actor", "on", "send",
            "true", "false", "none", "self",
            "test", "comptime", "with", "break", "continue", "loop", "extern"
        ];
        self.lexeme == kw && keywords.contains(&self.lexeme.as_str())
    }
    
    pub fn is_ident(&self) -> bool {
        let keywords = [
            "fn", "let", "var", "if", "else", "while", "for", "in",
            "return", "match", "struct", "enum", "impl", "use", "pub",
            "async", "await", "spawn", "actor", "on", "send",
            "true", "false", "none", "self",
            "test", "comptime", "with", "break", "continue", "loop", "extern"
        ];
        if self.lexeme.is_empty() {
            return false;
        }
        let c = self.lexeme.chars().next().unwrap();
        let is_alpha = c.is_ascii_alphabetic() || c == '_';
        is_alpha && !keywords.contains(&self.lexeme.as_str())
    }
    
    pub fn is_error(&self) -> bool {
        matches!(self.kind, TokenKind::Error(_))
    }
}

// =============================================================================
// Lexer
// =============================================================================

/// The KORE lexer - converts source text to tokens
pub struct Lexer {
    source: String,
    chars: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
    indent_stack: Vec<usize>,
}

impl Lexer {
    /// Create a new lexer for the given source code
    pub fn new(source: String) -> Lexer {
        let chars: Vec<char> = source.chars().collect();
        Lexer {
            source,
            chars,
            pos: 0,
            line: 1,
            column: 1,
            indent_stack: vec![0],
        }
    }
    
    /// Check if we've reached end of input
    fn is_eof(&self) -> bool {
        self.pos >= self.chars.len()
    }
    
    /// Get current character without advancing
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }
    
    /// Get character at offset without advancing
    fn peek_n(&self, n: usize) -> Option<char> {
        self.chars.get(self.pos + n).copied()
    }
    
    /// Advance and return current character
    fn advance(&mut self) -> Option<char> {
        if self.is_eof() {
            return None;
        }
        let c = self.chars[self.pos];
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(c)
    }
    
    /// Skip whitespace (not newlines - those are significant!)
    fn skip_spaces(&mut self) {
        while let Some(c) = self.peek() {
            if c == ' ' || c == '\r' {
                self.advance();
            } else {
                break;
            }
        }
    }
    
    /// Make a token at current position
    fn make_token(&self, kind: TokenKind, lexeme: &str) -> Token {
        Token::new(kind, self.line, self.column, lexeme.to_string())
    }
    
    /// Tokenize a number literal
    fn lex_number(&mut self) -> Token {
        let start = self.pos;
        let start_col = self.column;
        
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.advance();
            } else {
                break;
            }
        }
        
        // Check for float
        if self.peek() == Some('.') && self.peek_n(1).map(|c| c.is_ascii_digit()).unwrap_or(false) {
            self.advance(); // consume '.'
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    self.advance();
                } else {
                    break;
                }
            }
            let lexeme: String = self.chars[start..self.pos].iter().collect();
            let value: f64 = lexeme.parse().unwrap_or(0.0);
            return Token::new(TokenKind::Float(value), self.line, start_col, lexeme);
        }
        
        let lexeme: String = self.chars[start..self.pos].iter().collect();
        let value: i64 = lexeme.parse().unwrap_or(0);
        Token::new(TokenKind::Int(value), self.line, start_col, lexeme)
    }
    
    /// Tokenize a string literal
    fn lex_string(&mut self) -> Token {
        let start_col = self.column;
        let quote = self.advance().unwrap(); // consume opening quote
        let mut value = String::new();
        
        while let Some(c) = self.peek() {
            if c == quote {
                break;
            }
            self.advance();
            if c == '\\' {
                // Escape sequence
                if let Some(next) = self.advance() {
                    match next {
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),
                        '\\' => value.push('\\'),
                        '"' => value.push('"'),
                        '\'' => value.push('\''),
                        _ => value.push(next),
                    }
                }
            } else {
                value.push(c);
            }
        }
        
        self.advance(); // consume closing quote
        let lexeme = format!("\"{}\"", value);
        Token::new(TokenKind::String(value), self.line, start_col, lexeme)
    }
    
    /// Tokenize an identifier or keyword
    fn lex_ident(&mut self) -> Token {
        let start = self.pos;
        let start_col = self.column;
        
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        
        let lexeme: String = self.chars[start..self.pos].iter().collect();
        
        // Check for keywords
        let keywords = [
            "fn", "let", "var", "if", "else", "while", "for", "in",
            "return", "match", "struct", "enum", "impl", "use", "pub",
            "async", "await", "spawn", "actor", "on", "send",
            "true", "false", "none", "self",
            "test", "comptime", "with", "break", "continue", "loop", "extern"
        ];
        
        if keywords.contains(&lexeme.as_str()) {
            if lexeme == "true" {
                return Token::new(TokenKind::Bool(true), self.line, start_col, lexeme);
            } else if lexeme == "false" {
                return Token::new(TokenKind::Bool(false), self.line, start_col, lexeme);
            }
            return Token::new(TokenKind::Keyword(lexeme.clone()), self.line, start_col, lexeme);
        }
        
        Token::new(TokenKind::Ident(lexeme.clone()), self.line, start_col, lexeme)
    }
    
    /// Get the next token (low-level, doesn't handle indentation automatically)
    pub fn next_token(&mut self) -> Token {
        // Removed automatic skip_spaces() to handle indentation in tokenize()

        
        if self.is_eof() {
            return self.make_token(TokenKind::Eof, "");
        }
        
        let c = self.peek().unwrap();
        
        // Single-line comment
        if c == '#' || (c == '/' && self.peek_n(1) == Some('/')) {
            while let Some(ch) = self.peek() {
                if ch == '\n' {
                    break;
                }
                self.advance();
            }
            return self.next_token();
        }
        
        // Skip carriage return (Windows CRLF line endings)
        if c == '\r' {
            self.advance();
            return self.next_token();
        }
        
        // Newline (significant!)
        if c == '\n' {
            self.advance();
            return self.make_token(TokenKind::Newline, "\n");
        }
        
        // Numbers
        if c.is_ascii_digit() {
            return self.lex_number();
        }
        
        // Strings
        if c == '"' || c == '\'' {
            return self.lex_string();
        }
        
        // Identifiers and keywords
        if c.is_ascii_alphabetic() || c == '_' {
            return self.lex_ident();
        }
        
        // Operators and punctuation
        self.advance();
        
        match c {
            '+' => self.make_token(TokenKind::Plus, "+"),
            '-' => {
                if self.peek() == Some('>') {
                    self.advance();
                    self.make_token(TokenKind::Arrow, "->")
                } else {
                    self.make_token(TokenKind::Minus, "-")
                }
            }
            '*' => self.make_token(TokenKind::Star, "*"),
            '/' => self.make_token(TokenKind::Slash, "/"),
            '%' => self.make_token(TokenKind::Percent, "%"),
            '=' => {
                if self.peek() == Some('=') {
                    self.advance();
                    self.make_token(TokenKind::EqEq, "==")
                } else if self.peek() == Some('>') {
                    self.advance();
                    self.make_token(TokenKind::FatArrow, "=>")
                } else {
                    self.make_token(TokenKind::Eq, "=")
                }
            }
            '!' => {
                if self.peek() == Some('=') {
                    self.advance();
                    self.make_token(TokenKind::NotEq, "!=")
                } else {
                    self.make_token(TokenKind::Not, "!")
                }
            }
            '<' => {
                if self.peek() == Some('=') {
                    self.advance();
                    self.make_token(TokenKind::LtEq, "<=")
                } else {
                    self.make_token(TokenKind::Lt, "<")
                }
            }
            '>' => {
                if self.peek() == Some('=') {
                    self.advance();
                    self.make_token(TokenKind::GtEq, ">=")
                } else {
                    self.make_token(TokenKind::Gt, ">")
                }
            }
            '&' => {
                if self.peek() == Some('&') {
                    self.advance();
                    self.make_token(TokenKind::And, "&&")
                } else {
                    self.make_token(TokenKind::Ampersand, "&")
                }
            }
            '|' => {
                if self.peek() == Some('|') {
                    self.advance();
                    self.make_token(TokenKind::Or, "||")
                } else {
                    self.make_token(TokenKind::Pipe, "|")
                }
            }
            '.' => {
                if self.peek() == Some('.') {
                    self.advance();
                    self.make_token(TokenKind::DotDot, "..")
                } else {
                    self.make_token(TokenKind::Dot, ".")
                }
            }
            ':' => {
                if self.peek() == Some(':') {
                    self.advance();
                    self.make_token(TokenKind::ColonColon, "::")
                } else {
                    self.make_token(TokenKind::Colon, ":")
                }
            }
            ',' => self.make_token(TokenKind::Comma, ","),
            ';' => self.make_token(TokenKind::Semicolon, ";"),
            '(' => self.make_token(TokenKind::LParen, "("),
            ')' => self.make_token(TokenKind::RParen, ")"),
            '[' => self.make_token(TokenKind::LBracket, "["),
            ']' => self.make_token(TokenKind::RBracket, "]"),
            '{' => self.make_token(TokenKind::LBrace, "{"),
            '}' => self.make_token(TokenKind::RBrace, "}"),
            _ => self.make_token(TokenKind::Error(format!("Unknown character: {}", c)), &c.to_string()),
        }
    }
    
    /// Tokenize entire source into array of tokens with Indent/Dedent injection
    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut at_start_of_line = true;
        
        loop {
            // 1. Skip spaces (but measure if at start of line)
            let mut spaces = 0;
            while let Some(c) = self.peek() {
                if c == ' ' {
                    spaces += 1;
                    self.advance();
                } else if c == '\t' {
                    spaces += 4;
                    self.advance();
                } else if c == '\r' {
                    self.advance();
                } else {
                    break;
                }
            }
            
            // 2. Handle comments (they act like whitespace)
            if self.peek() == Some('#') || (self.peek() == Some('/') && self.peek_n(1) == Some('/')) {
                while let Some(c) = self.peek() {
                    if c == '\n' { break; }
                    self.advance();
                }
                // Comment ends at newline, loop back to handle newline
                continue; 
            }

            // 3. Handle Newline / Indentation
            if self.peek() == Some('\n') {
                self.advance(); // consume \n
                tokens.push(self.make_token(TokenKind::Newline, "\n"));
                at_start_of_line = true;
                continue;
            }
            
            // 4. If we are at start of line and see non-whitespace, handle Indent/Dedent
            if at_start_of_line && !self.is_eof() {
                let current_indent = spaces;
                let last_indent = *self.indent_stack.last().unwrap_or(&0);
                
                if current_indent > last_indent {
                    self.indent_stack.push(current_indent);
                    tokens.push(self.make_token(TokenKind::Indent, "    "));
                } else if current_indent < last_indent {
                    while *self.indent_stack.last().unwrap_or(&0) > current_indent {
                        self.indent_stack.pop();
                        tokens.push(self.make_token(TokenKind::Dedent, ""));
                    }
                    if *self.indent_stack.last().unwrap_or(&0) != current_indent {
                        tokens.push(self.make_token(TokenKind::Error("Indentation mismatch".to_string()), ""));
                    }
                }
                at_start_of_line = false;
            }
            
            // 5. Normal tokenization
            if self.is_eof() {
                // Emit remaining Dedents
                while self.indent_stack.len() > 1 {
                    self.indent_stack.pop();
                    tokens.push(self.make_token(TokenKind::Dedent, ""));
                }
                tokens.push(self.make_token(TokenKind::Eof, ""));
                break;
            }
            
            let tok = self.next_token();
            tokens.push(tok);
        }
        tokens
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic_tokens() {
        let source = "fn main(): let x = 42";
        let mut lexer = Lexer::new(source.to_string());
        let tokens = lexer.tokenize();
        assert!(tokens.len() > 0);
    }
    
    #[test]
    fn test_operators() {
        let source = "+ - * / == != < > <= >= -> =>";
        let mut lexer = Lexer::new(source.to_string());
        let tokens = lexer.tokenize();
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Arrow)));
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::FatArrow)));
    }
}
