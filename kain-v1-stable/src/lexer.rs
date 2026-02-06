//! KAIN Lexer - Significant whitespace + Rust-like tokens
//!
//! Key features:
//! - Python-style indentation (INDENT/DEDENT tokens)
//! - Rust-style identifiers and literals
//! - JSX-style angle brackets for UI
//! - Effect annotations with `with` keyword

use logos::Logos;
use crate::span::Span;
use crate::error::{KainError, KainResult};

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r]+")]  // Skip horizontal whitespace AND carriage returns
pub enum TokenKind {
    // === Keywords ===
    #[token("fn")]
    Fn,
    #[token("let")]
    Let,
    #[token("mut")]
    Mut,
    #[token("var")]
    Var,
    #[token("const")]
    Const,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("elif")]
    Elif,
    #[token("match")]
    Match,
    #[token("for")]
    For,
    #[token("while")]
    While,
    #[token("loop")]
    Loop,
    #[token("break")]
    Break,
    #[token("continue")]
    Continue,
    #[token("return")]
    Return,
    #[token("await")]
    Await,
    #[token("in")]
    In,
    #[token("with")]
    With,
    #[token("as")]
    As,
    #[token("type")]
    TypeKw,
    #[token("struct")]
    Struct,
    #[token("enum")]
    Enum,
    #[token("trait")]
    Trait,
    #[token("impl")]
    Impl,
    #[token("pub")]
    Pub,
    #[token("mod")]
    Mod,
    #[token("use")]
    Use,
    #[token("self")]
    SelfLower,
    #[token("Self")]
    SelfUpper,
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("none")]
    None,

    // === Special Keywords (First-Class Citizens) ===
    #[token("component")]
    Component,
    #[token("shader")]
    Shader,
    #[token("actor")]
    Actor,
    #[token("spawn")]
    Spawn,
    #[token("send")]
    Send,
    #[token("receive")]
    Receive,
    #[token("emit")]
    Emit,
    #[token("comptime")]
    Comptime,
    #[token("macro")]
    Macro,
    #[token("vertex")]
    Vertex,
    #[token("fragment")]
    Fragment,

    // === Testing ===
    #[token("test")]
    Test,
    // Note: 'compute' is NOT a keyword - it's handled as an identifier in shader contexts

    // === Effect Keywords ===
    #[token("Pure")]
    Pure,
    #[token("IO")]
    Io,
    #[token("async")]  // lowercase for 'async fn' syntax
    AsyncKw,
    #[token("Async")]  // capital for 'with Async' effect syntax
    Async,
    #[token("GPU")]
    Gpu,
    #[token("Reactive")]
    Reactive,
    #[token("Unsafe")]
    Unsafe,

    // === Literals ===
    #[regex(r"[0-9][0-9_]*", |lex| lex.slice().replace('_', "").parse().ok())]
    Int(i64),

    #[regex(r"[0-9][0-9_]*\.[0-9][0-9_]*", |lex| lex.slice().replace('_', "").parse().ok())]
    Float(f64),

    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        Some(unescape(&s[1..s.len()-1]))
    })]
    String(String),

    #[regex(r#"f"([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        // For f-strings, we currently don't unescape everything because brace handling is complex,
        // but we SHOULD unescape quotes at least.
        // For now, let's treat f-strings same as strings but knowing formatting comes later.
        // Actually, if we unescape, we might break {variable} if the user wrote \{variable} (escaped brace).
        // Let's leave FString raw for now or handle it carefully.
        // The user issue is specifically about standard strings.
        Some(s[2..s.len()-1].to_string())
    })]
    FString(String),

    #[regex(r#"'([^'\\]|\\.)*'"#, |lex| {
        let s = lex.slice();
        Some(unescape(&s[1..s.len()-1]))
    })]
    Char(String),

    // === Identifiers ===
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),

    // === Operators ===
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("**")]
    Power,
    #[token("==")]
    EqEq,
    #[token("!=")]
    NotEq,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("<=")]
    LtEq,
    #[token(">=")]
    GtEq,
    #[token("&&")]
    And,
    #[token("||")]
    Or,
    #[token("!")]
    Not,
    #[token("&")]
    Amp,
    #[token("|")]
    Pipe,
    #[token("^")]
    Caret,
    #[token("~")]
    Tilde,
    #[token("<<")]
    Shl,
    #[token(">>")]
    Shr,

    // === Assignment ===
    #[token("=")]
    Eq,
    #[token("+=")]
    PlusEq,
    #[token("-=")]
    MinusEq,
    #[token("*=")]
    StarEq,
    #[token("/=")]
    SlashEq,

    // === Punctuation ===
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token(",")]
    Comma,
    #[token(".")]
    Dot,
    #[token("..")]
    DotDot,
    #[token("...")]
    DotDotDot,
    #[token(":")]
    Colon,
    #[token("::")]
    ColonColon,
    #[token(";")]
    Semi,
    #[token("->")]
    Arrow,
    #[token("=>")]
    FatArrow,
    #[token("@")]
    At,
    #[token("?")]
    Question,

    // === JSX-like ===
    #[token("</")]
    LtSlash,

    // === Whitespace (significant!) ===
    #[regex(r"\n[ \t]*", |lex| lex.slice().to_string())]
    Newline(String),

    #[regex(r"//[^\n]*", priority = 3)]
    Comment,

    #[regex(r"#[^\n]*", priority = 2)]
    HashComment,

    // Synthetic tokens (inserted during indent processing)
    Indent,
    Dedent,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

pub struct Lexer<'a> {
    source: &'a str,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self { source }
    }

    pub fn tokenize(&self) -> KainResult<Vec<Token>> {
        let mut lex = TokenKind::lexer(self.source);
        let mut raw_tokens = Vec::new();

        while let Some(result) = lex.next() {
            let span = Span::new(lex.span().start, lex.span().end);
            match result {
                Ok(kind) => {
                    // Skip comments
                    if matches!(kind, TokenKind::Comment | TokenKind::HashComment) {
                        continue;
                    }
                    raw_tokens.push(Token::new(kind, span));
                }
                Err(_) => {
                    return Err(KainError::lexer(
                        format!("Unexpected character: '{}'", &self.source[span.start..span.end]),
                        span,
                    ));
                }
            }
        }

        // Process indentation
        let tokens = self.process_indentation(raw_tokens)?;
        Ok(tokens)
    }

    /// Convert newlines with leading whitespace into INDENT/DEDENT tokens
    fn process_indentation(&self, raw: Vec<Token>) -> KainResult<Vec<Token>> {
        let mut result = Vec::new();
        let mut indent_stack: Vec<usize> = vec![0]; // Stack of indent levels
        let mut iter = raw.into_iter().peekable();

        while let Some(token) = iter.next() {
            match &token.kind {
                TokenKind::Newline(ws) => {
                    // Check if this is a blank line (followed by another newline)
                    if let Some(next) = iter.peek() {
                        if matches!(next.kind, TokenKind::Newline(_)) {
                            continue;
                        }
                    }

                    // Calculate indent level (count spaces, tabs = 4 spaces)
                    let indent: usize = ws[1..].chars().map(|c| if c == '\t' { 4 } else { 1 }).sum();
                    let current = *indent_stack.last().unwrap();

                    if indent > current {
                        // Increased indent
                        indent_stack.push(indent);
                        result.push(Token::new(TokenKind::Newline(ws.clone()), token.span));
                        result.push(Token::new(TokenKind::Indent, token.span));
                    } else if indent < current {
                        // Decreased indent - may produce multiple DEDENTs
                        result.push(Token::new(TokenKind::Newline(ws.clone()), token.span));
                        while indent_stack.len() > 1 && *indent_stack.last().unwrap() > indent {
                            indent_stack.pop();
                            result.push(Token::new(TokenKind::Dedent, token.span));
                        }
                    } else {
                        // Same indent level
                        result.push(Token::new(TokenKind::Newline(ws.clone()), token.span));
                    }
                }
                _ => {
                    result.push(token);
                }
            }
        }

        // Close remaining indents
        let final_span = result.last().map(|t| t.span).unwrap_or(Span::new(0, 0));
        while indent_stack.len() > 1 {
            indent_stack.pop();
            result.push(Token::new(TokenKind::Dedent, final_span));
        }

        result.push(Token::new(TokenKind::Eof, final_span));
        Ok(result)
    }
}

fn unescape(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('0') => result.push('\0'),
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('\'') => result.push('\''),
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tokens() {
        let source = "fn factorial(n: Int) -> Int";
        let tokens = Lexer::new(source).tokenize().unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::Fn));
        assert!(matches!(tokens[1].kind, TokenKind::Ident(_)));
    }

    #[test]
    fn test_indentation() {
        let source = "fn foo():\n    let x = 1\n    let y = 2\n";
        let tokens = Lexer::new(source).tokenize().unwrap();
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Indent)));
    }
}

