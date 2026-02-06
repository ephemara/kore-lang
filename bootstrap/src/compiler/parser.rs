// ============================================================================
// KAIN Bootstrap Compiler - Parser (Rust)
// ============================================================================
// Hand-translated from parser.kn for Project Ouroboros bootstrap
// This is the self-hosted KAIN parser running as native Rust code.
// ============================================================================

use crate::compiler::lexer::{Token, TokenKind, Lexer};

// =============================================================================
// AST Node Types
// =============================================================================

/// A KAIN program is a list of top-level items
#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Item>,
}

/// Top-level items
#[derive(Debug, Clone)]
pub enum Item {
    Function(FnDef),
    Struct(StructDef),
    Enum(EnumDef),
    Impl(ImplDef),
    Use(String),
    Extern(ExternFnDef),
}

/// External function definition
#[derive(Debug, Clone)]
pub struct ExternFnDef {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<String>,
}

/// Function definition
#[derive(Debug, Clone)]
pub struct FnDef {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<String>,
    pub body: Vec<Stmt>,
    pub is_pub: bool,
    pub is_async: bool,
}

/// Function parameter
#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Option<String>,
}

/// Struct definition
#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<Field>,
    pub is_pub: bool,
}

/// Struct field
#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: String,
}

/// Enum definition
#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<Variant>,
}

/// Enum variant
#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<String>,
}

/// Impl block
#[derive(Debug, Clone)]
pub struct ImplDef {
    pub target: String,
    pub methods: Vec<FnDef>,
}

// =============================================================================
// Statements
// =============================================================================

#[derive(Debug, Clone)]
pub enum Stmt {
    Let(String, Option<String>, Expr),
    Var(String, Option<String>, Expr),
    Assign(Expr, Expr),
    Return(Option<Expr>),
    If(Expr, Vec<Stmt>, Option<Vec<Stmt>>),
    While(Expr, Vec<Stmt>),
    Loop(Vec<Stmt>),
    For(String, Expr, Vec<Stmt>),
    Match(Expr, Vec<MatchArm>),
    Expr(Expr),
    Break,
    Continue,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Wildcard,
    Ident(String),
    Literal(Expr),
    Variant(Option<String>, String, Vec<Pattern>),
}

// =============================================================================
// Expressions
// =============================================================================

#[derive(Debug, Clone)]
pub enum Expr {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    None,
    Ident(String),
    Binary(Box<Expr>, String, Box<Expr>),
    Unary(String, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    MethodCall(Box<Expr>, String, Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Field(Box<Expr>, String),
    Tuple(Vec<Expr>),
    Array(Vec<Expr>),
    Struct(String, Vec<FieldInit>),
    EnumVariant(String, String, Vec<Expr>),
    If(Box<Expr>, Box<Expr>, Option<Box<Expr>>),
    Lambda(Vec<String>, Box<Expr>),
    Await(Box<Expr>),
}

#[derive(Debug, Clone)]
pub struct FieldInit {
    pub name: String,
    pub value: Expr,
}

// =============================================================================
// Parser
// =============================================================================

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Parser {
        Parser { tokens, pos: 0 }
    }
    
    fn is_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }
    
    fn peek(&self) -> Token {
        if self.is_eof() {
            Token::new(TokenKind::Eof, 0, 0, String::new())
        } else {
            self.tokens[self.pos].clone()
        }
    }
    
    fn peek_n(&self, n: usize) -> Token {
        let idx = self.pos + n;
        if idx >= self.tokens.len() {
            Token::new(TokenKind::Eof, 0, 0, String::new())
        } else {
            self.tokens[idx].clone()
        }
    }
    
    fn advance(&mut self) -> Token {
        let tok = self.peek();
        self.pos += 1;
        tok
    }
    
    fn expect_keyword(&mut self, kw: &str) -> Token {
        let tok = self.advance();
        if !tok.is_keyword(kw) {
            panic!("Expected keyword '{}', got: {}", kw, tok.lexeme);
        }
        tok
    }
    
    fn expect(&mut self, _kind_name: &str) -> Token {
        self.advance()
    }
    
    fn skip_newlines(&mut self) {
        while !self.is_eof() {
            let tok = self.peek();
            if tok.lexeme == "\n" || tok.lexeme == "\r\n" || tok.lexeme == "\r" {
                self.advance();
            } else {
                break;
            }
        }
    }
    
    /// Parse a complete program
    pub fn parse_program(&mut self) -> Program {
        let mut items = Vec::new();
        
        self.skip_newlines();
        
        self.skip_newlines(); // Skip leading newlines (explicit tokens now)
        while !self.is_eof() {
            if matches!(self.peek().kind, TokenKind::Eof) {
                break;
            }
            if matches!(self.peek().kind, TokenKind::Dedent) {
                self.advance();
                continue;
            }
            
            let item = self.parse_item();
            items.push(item);
            self.skip_newlines();
        }
        
        Program { items }
    }
    
    /// Parse a top-level item
    fn parse_item(&mut self) -> Item {
        // Skip any stray / tokens (from doc comments that weren't fully consumed)
        while self.peek().lexeme == "/" {
            self.advance();
            // Skip to end of line
            while !self.is_eof() && self.peek().lexeme != "\n" {
                self.advance();
            }
            self.skip_newlines();
        }
        
        let tok = self.peek();
        
        if tok.is_keyword("fn") {
            Item::Function(self.parse_fn_def(false, false))
        } else if tok.is_keyword("pub") {
            self.advance();
            let next = self.peek();
            if next.is_keyword("fn") {
                Item::Function(self.parse_fn_def(true, false))
            } else if next.is_keyword("struct") {
                Item::Struct(self.parse_struct_def(true))
            } else {
                panic!("Expected fn or struct after pub");
            }
        } else if tok.is_keyword("async") {
            self.advance();
            self.expect_keyword("fn");
            Item::Function(self.parse_fn_def(false, true))
        } else if tok.is_keyword("struct") {
            Item::Struct(self.parse_struct_def(false))
        } else if tok.is_keyword("enum") {
            Item::Enum(self.parse_enum_def())
        } else if tok.is_keyword("impl") {
            Item::Impl(self.parse_impl_def())
        } else if tok.is_keyword("use") {
            Item::Use(self.parse_use())
        } else if tok.is_keyword("extern") {
            Item::Extern(self.parse_extern())
        } else {
            panic!("Unexpected token at top level: {:?} at line {}:{}", tok.kind, tok.line, tok.column);
        }
    }
    
    /// Parse function definition
    fn parse_fn_def(&mut self, is_pub: bool, is_async: bool) -> FnDef {
        self.expect_keyword("fn");
        
        let name_tok = self.advance();
        let name = name_tok.lexeme;
        
        // Parse parameters
        self.expect("(");
        let params = self.parse_params();
        self.expect(")");
        
        // Parse return type if present
        let mut return_type = None;
        if self.peek().lexeme == "->" {
            self.advance(); // consume '->'
            return_type = Some(self.parse_type());
        }
        
        // Skip 'with Effect' if present
        if self.peek().is_keyword("with") {
            self.advance(); // consume 'with'
            self.advance(); // consume effect name
        }
        
        // Parse body
        self.expect(":");
        self.skip_newlines();
        let body = self.parse_block();
        
        FnDef {
            name,
            params,
            return_type,
            body,
            is_pub,
            is_async,
        }
    }
    
    fn parse_params(&mut self) -> Vec<Param> {
        let mut params = Vec::new();
        
        while !self.is_eof() {
            let tok = self.peek();
            
            // Stop at closing paren
            if tok.lexeme == ")" {
                break;
            }
            
            // Handle 'self' parameter
            if tok.is_keyword("self") {
                self.advance();
                params.push(Param { name: "self".to_string(), ty: Some("Self".to_string()) });
                if self.peek().lexeme == "," {
                    self.advance();
                }
                continue;
            }
            
            // Check if it's an identifier
            if tok.is_ident() {
                self.advance();
                let param_name = tok.lexeme;
                let mut param_type = None;
                
                // Check for type annotation
                if self.peek().lexeme == ":" {
                    self.advance(); // consume ':'
                    param_type = Some(self.parse_type());
                }
                
                params.push(Param { name: param_name, ty: param_type });
                
                // Check for comma
                if self.peek().lexeme == "," {
                    self.advance();
                }
            } else {
                break;
            }
        }
        
        params
    }
    
    /// Parse a type (handles Array<T>, Option<T>, etc.)
    fn parse_type(&mut self) -> String {
        let base = self.advance().lexeme;
        
        // Check for generic parameters
        if self.peek().lexeme == "<" {
            self.advance(); // consume '<'
            let mut inner = self.parse_type();
            
            // Handle nested generics like Option<Box<Expr>>
            while self.peek().lexeme == "," {
                self.advance();
                inner = format!("{}, {}", inner, self.parse_type());
            }
            
            self.advance(); // consume '>'
            return format!("{}<{}>", base, inner);
        }
        
        base
    }
    
    fn parse_block(&mut self) -> Vec<Stmt> {
        let mut stmts = Vec::new();
        let max_stmts = 10000; // Safety limit
        
        // Check for indentation
        let has_indent = if matches!(self.peek().kind, TokenKind::Indent) {
            self.advance(); // Consume Indent
            true
        } else {
            false
        };
        
        while !self.is_eof() && stmts.len() < max_stmts {
            let tok = self.peek();
            
            // Handle dedent if we are in an indented block
            if has_indent && matches!(tok.kind, TokenKind::Dedent) {
                self.advance(); // Consume Dedent
                break;
            }
            
            // Skip newlines
            if matches!(tok.kind, TokenKind::Newline) {
                self.advance();
                continue;
            }
            
            // Handle EOF inside block
            if matches!(tok.kind, TokenKind::Eof) {
                break;
            }

            // If not indented, and we hit a terminator (newline or dedent or EOF), break after one stmt
            // But wait, if inline, we parse ONE statement and return.
            
            // Parse a statement
            let start_pos = self.pos;
            let stmt = self.parse_stmt();
            stmts.push(stmt);
            
            // Safety: if position didn't advance, break to avoid infinite loop
            if self.pos == start_pos {
                break; 
            }
            
            // If we are NOT indented, we only parse ONE statement (inline block)
            // e.g. "if true: return"
            if !has_indent {
                break;
            }
        }
        
        stmts
    }
    
    fn parse_stmt(&mut self) -> Stmt {
        let tok = self.peek();
        
        if tok.is_error() {
            self.advance();
            return Stmt::Expr(Expr::None);
        }
        
        if tok.is_keyword("let") {
            return self.parse_let();
        }
        
        if tok.is_keyword("var") {
            return self.parse_var();
        }
        
        if tok.is_keyword("return") {
            return self.parse_return();
        }
        
        if tok.is_keyword("if") {
            return self.parse_if();
        }
        
        if tok.is_keyword("while") {
            return self.parse_while();
        }
        
        if tok.is_keyword("for") {
            return self.parse_for();
        }
        
        if tok.is_keyword("match") {
            return self.parse_match();
        }
        
        if tok.is_keyword("break") {
            self.advance();
            return Stmt::Break;
        }
        
        if tok.is_keyword("continue") {
            self.advance();
            return Stmt::Continue;
        }
        
        if tok.is_keyword("loop") {
            return self.parse_loop();
        }
        
        // Expression statement
        let expr = self.parse_expr();
        
        // Check for assignment
        if self.peek().lexeme == "=" {
            self.advance();
            let value = self.parse_expr();
            return Stmt::Assign(expr, value);
        }
        
        Stmt::Expr(expr)
    }
    
    fn parse_let(&mut self) -> Stmt {
        self.expect_keyword("let");
        let name_tok = self.advance();
        let name = name_tok.lexeme;
        self.expect("=");
        let value = self.parse_expr();
        Stmt::Let(name, None, value)
    }
    
    fn parse_var(&mut self) -> Stmt {
        self.expect_keyword("var");
        let name_tok = self.advance();
        let name = name_tok.lexeme;
        self.expect("=");
        let value = self.parse_expr();
        Stmt::Var(name, None, value)
    }
    
    fn parse_return(&mut self) -> Stmt {
        self.expect_keyword("return");
        if self.peek().lexeme == "\n" {
            return Stmt::Return(None);
        }
        let expr = self.parse_expr();
        Stmt::Return(Some(expr))
    }
    
    fn parse_if(&mut self) -> Stmt {
        self.expect_keyword("if");
        let cond = self.parse_expr();
        self.expect(":");
        self.skip_newlines();
        let then_block = self.parse_block();
        
        self.skip_newlines();
        let mut else_block = None;
        if self.peek().is_keyword("else") {
            self.advance();
            
            // Handle "else if"
            if self.peek().is_keyword("if") {
                let if_stmt = self.parse_if();
                else_block = Some(vec![if_stmt]);
            } else {
                self.expect(":");
                self.skip_newlines();
                else_block = Some(self.parse_block());
            }
        }
        
        Stmt::If(cond, then_block, else_block)
    }
    
    fn parse_while(&mut self) -> Stmt {
        self.expect_keyword("while");
        let cond = self.parse_expr();
        self.expect(":");
        self.skip_newlines();
        let body = self.parse_block();
        Stmt::While(cond, body)
    }
    
    fn parse_for(&mut self) -> Stmt {
        self.expect_keyword("for");
        let var_tok = self.advance();
        let var_name = var_tok.lexeme;
        self.expect_keyword("in");
        let iter = self.parse_expr();
        self.expect(":");
        self.skip_newlines();
        let body = self.parse_block();
        Stmt::For(var_name, iter, body)
    }
    
    fn parse_loop(&mut self) -> Stmt {
        self.expect_keyword("loop");
        self.expect(":");
        self.skip_newlines();
        let body = self.parse_block();
        Stmt::Loop(body)
    }
    
    fn parse_match(&mut self) -> Stmt {
        self.expect_keyword("match");
        let scrutinee = self.parse_expr();
        self.expect(":");
        self.skip_newlines();
        
        let mut arms = Vec::new();
        
        // Check indentation
        let has_indent = if matches!(self.peek().kind, TokenKind::Indent) {
            self.advance(); true
        } else {
            false
        };
        
        while !self.is_eof() {
            let tok = self.peek();
            
            if has_indent && matches!(tok.kind, TokenKind::Dedent) {
                self.advance();
                break;
            }
            
            // Stop at new statement-level keywords if NOT indented
            if !has_indent {
                if tok.is_keyword("fn") || tok.is_keyword("let") || tok.is_keyword("var") 
                   || tok.is_keyword("return") || tok.is_keyword("if") 
                   || tok.is_keyword("while") || tok.is_keyword("for") {
                    break;
                }
                if tok.is_keyword("struct") || tok.is_keyword("enum") || tok.is_keyword("impl") 
                   || tok.is_keyword("use") || tok.is_keyword("pub") {
                    break;
                }
            }
            
            // Skip newlines
            if matches!(tok.kind, TokenKind::Newline) {
                self.advance();
                continue;
            }
            
            // Parse pattern => body
            let pattern = self.parse_pattern();
            // Expect => (either as single token "=>" or two tokens "=" + ">")
            let arrow_tok = self.peek();
            if arrow_tok.kind == TokenKind::FatArrow {
                self.advance(); // consume single "=>" token
            } else if arrow_tok.lexeme == "=" {
                self.advance(); // consume "="
                if self.peek().lexeme == ">" {
                    self.advance(); // consume ">"
                }
            } else {
                // Skip if neither - might be a recovery case
                self.advance();
            }
            
            // Parse arm body
            let body = if matches!(self.peek().kind, TokenKind::Newline) {
                self.skip_newlines();
                self.parse_block()
            } else if matches!(self.peek().kind, TokenKind::LBrace) {
                // Legacy brace support or just expression block
                // Actually parse_block handles generic blocks now? 
                // Wait, parse_expr handles braces? No. 
                // Let's assume block if newline, else expr-as-stmt
                self.parse_block()
            } else {
                // Inline body: => expr
                // We parse one expression and wrap it in Stmt::Expr
                // But wait, an arm body IS a Vec<Stmt> (block)
                // If it's a single expr, we wrap it.
                // But parse_block logic handles "if not indent, parse one stmt".
                // parse_stmt -> Stmt::Expr -> Expr
                // So calling parse_block() will handle "expr" as a single statement block!
                // PERFECT.
                self.parse_block()
            };
            
            arms.push(MatchArm { pattern, body });
            
            if !has_indent { 
                // Inline match? Only support one arm? Or loop? 
                // Usually inline match isn't a thing, but let's allow loop.
                // If implied end by keyword, loop breaks.
            }
        }
        
        Stmt::Match(scrutinee, arms)
    }
    
    fn parse_pattern(&mut self) -> Pattern {
        let tok = self.peek();
        
        // Wildcard pattern
        if tok.lexeme == "_" {
            self.advance();
            return Pattern::Wildcard;
        }
        
        // Identifier or variant pattern
        if tok.is_ident() {
            self.advance();
            let mut name = tok.lexeme.clone();
            let mut enum_prefix = None;
            
            // Handle qualified names like Target::LLVM
            if self.peek().lexeme == "::" {
                self.advance(); // consume '::'
                let variant_name = self.advance().lexeme;
                enum_prefix = Some(name);
                name = variant_name;
            }
            
            // Check for variant with bindings like Some(x) or Variant(a, b)
            if self.peek().lexeme == "(" {
                self.advance(); // consume '('
                let mut bindings = Vec::new();
                let max_bindings = 100;
                while !self.is_eof() && self.peek().lexeme != ")" && bindings.len() < max_bindings {
                    bindings.push(self.parse_pattern());
                    if self.peek().lexeme == "," {
                        self.advance();
                    } else if self.peek().lexeme != ")" {
                        break;
                    }
                }
                if !self.is_eof() && self.peek().lexeme == ")" {
                    self.advance(); // consume ')'
                }
                return Pattern::Variant(enum_prefix, name, bindings);
            }
            
            // If it's a known non-namespaced variant (like None/Some), it might still be a variant
            // We'll handle this in the codegen by checking the variant_map
            return Pattern::Ident(name);
        }
        
        // Literal pattern
        let expr = self.parse_primary();
        Pattern::Literal(expr)
    }
    
    // Expression parser with operator precedence
    fn parse_expr(&mut self) -> Expr {
        self.parse_or()
    }
    
    fn parse_or(&mut self) -> Expr {
        let mut left = self.parse_and();
        while self.peek().lexeme == "||" {
            self.advance();
            let right = self.parse_and();
            left = Expr::Binary(Box::new(left), "||".to_string(), Box::new(right));
        }
        left
    }
    
    fn parse_and(&mut self) -> Expr {
        let mut left = self.parse_equality();
        while self.peek().lexeme == "&&" {
            self.advance();
            let right = self.parse_equality();
            left = Expr::Binary(Box::new(left), "&&".to_string(), Box::new(right));
        }
        left
    }
    
    fn parse_equality(&mut self) -> Expr {
        let mut left = self.parse_comparison();
        while self.peek().lexeme == "==" || self.peek().lexeme == "!=" {
            let op = self.advance().lexeme;
            let right = self.parse_comparison();
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        left
    }
    
    fn parse_comparison(&mut self) -> Expr {
        let mut left = self.parse_term();
        while self.peek().lexeme == "<" || self.peek().lexeme == ">" 
              || self.peek().lexeme == "<=" || self.peek().lexeme == ">=" {
            let op = self.advance().lexeme;
            let right = self.parse_term();
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        left
    }
    
    fn parse_term(&mut self) -> Expr {
        let mut left = self.parse_factor();
        while self.peek().lexeme == "+" || self.peek().lexeme == "-" {
            let op = self.advance().lexeme;
            let right = self.parse_factor();
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        left
    }
    
    fn parse_factor(&mut self) -> Expr {
        let mut left = self.parse_unary();
        while self.peek().lexeme == "*" || self.peek().lexeme == "/" || self.peek().lexeme == "%" {
            let op = self.advance().lexeme;
            let right = self.parse_unary();
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        left
    }
    
    fn parse_unary(&mut self) -> Expr {
        if self.peek().lexeme == "!" || self.peek().lexeme == "-" {
            let op = self.advance().lexeme;
            let operand = self.parse_unary();
            return Expr::Unary(op, Box::new(operand));
        }
        self.parse_postfix()
    }
    
    fn parse_postfix(&mut self) -> Expr {
        let mut expr = self.parse_primary();
        let mut iterations = 0;
        let max_iterations = 10000;
        
        loop {
            if self.is_eof() || iterations >= max_iterations {
                break;
            }
            iterations += 1;
            
            let next = self.peek().lexeme.clone();
            
            if next == "." {
                self.advance(); // consume '.'
                let field = self.advance().lexeme;
                
                // Check if it's a method call
                if self.peek().lexeme == "(" {
                    self.advance(); // consume '('
                    let args = self.parse_args();
                    if !self.is_eof() && self.peek().lexeme == ")" {
                        self.advance(); // consume ')'
                    }
                    expr = Expr::MethodCall(Box::new(expr), field, args);
                } else {
                    expr = Expr::Field(Box::new(expr), field);
                }
                continue;
            }
            
            if next == "[" {
                self.advance(); // consume '['
                let index = self.parse_expr();
                if !self.is_eof() && self.peek().lexeme == "]" {
                    self.advance(); // consume ']'
                }
                expr = Expr::Index(Box::new(expr), Box::new(index));
                continue;
            }
            
            if next == "(" {
                self.advance(); // consume '('
                let args = self.parse_args();
                if !self.is_eof() && self.peek().lexeme == ")" {
                    self.advance(); // consume ')'
                }
                expr = Expr::Call(Box::new(expr), args);
                continue;
            }
            
            break;
        }
        
        expr
    }
    
    fn parse_args(&mut self) -> Vec<Expr> {
        let mut args = Vec::new();
        let max_args = 1000; // Safety limit
        while !self.is_eof() && self.peek().lexeme != ")" && args.len() < max_args {
            args.push(self.parse_expr());
            if self.peek().lexeme == "," {
                self.advance();
            } else if self.peek().lexeme != ")" {
                // Not a comma and not closing paren - might be stuck
                break;
            }
        }
        args
    }
    
    fn parse_primary(&mut self) -> Expr {
        let tok = self.peek();
        
        // Parenthesized expression
        // Parenthesized expression or Tuple
        if tok.lexeme == "(" {
            self.advance(); // consume '('
            
            // Empty tuple
            if self.peek().lexeme == ")" {
                self.advance();
                return Expr::Tuple(Vec::new());
            }
            
            let expr = self.parse_expr();
            
            // Check for comma -> Tuple
            if self.peek().lexeme == "," {
                let mut elements = vec![expr];
                while self.peek().lexeme == "," {
                    self.advance(); // consume ','
                    if self.peek().lexeme == ")" { break; }
                    elements.push(self.parse_expr());
                    
                    // Handle trailing comma or stuck loop
                    if self.peek().lexeme != "," && self.peek().lexeme != ")" {
                        // Error recovery: assumption is we should be seeing comma or )
                        // But let's verify next interpretation
                    }
                }
                if self.peek().lexeme == ")" {
                    self.advance();
                } else {
                    panic!("Expected ')' after tuple elements at {}:{}", self.peek().line, self.peek().column);
                }
                return Expr::Tuple(elements);
            }
            
            // Just a parenthesized expression
            if self.peek().lexeme == ")" {
                self.advance();
            } else {
                panic!("Expected ')' after expression at {}:{}", self.peek().line, self.peek().column);
            }
            return expr;
        }
        
        // Array literal
        if tok.lexeme == "[" {
            self.advance(); // consume '['
            
            // Skip leading newlines/indents
            while matches!(self.peek().kind, TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent) {
                self.advance();
            }
            
            let mut elements = Vec::new();
            let max_elements = 10000;
            while !self.is_eof() && self.peek().lexeme != "]" && elements.len() < max_elements {
                // Skip newlines/indents between elements
                while matches!(self.peek().kind, TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent) {
                    self.advance();
                }
                
                if self.peek().lexeme == "]" {
                    break;
                }
                
                elements.push(self.parse_expr());
                
                // Skip newlines/indents after element
                while matches!(self.peek().kind, TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent) {
                    self.advance();
                }
                
                if self.peek().lexeme == "," {
                    self.advance();
                    // Skip newlines/indents after comma
                    while matches!(self.peek().kind, TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent) {
                        self.advance();
                    }
                } else if self.peek().lexeme != "]" {
                    break; // Stuck, bail out
                }
            }
            if !self.is_eof() && self.peek().lexeme == "]" {
                self.advance(); // consume ']'
            }
            return Expr::Array(elements);
        }
        
        // Now consume the token
        self.advance();
        
        // Check for integer literal
        if !tok.lexeme.is_empty() {
            if let Some(first_char) = tok.lexeme.chars().next() {
                if first_char.is_ascii_digit() {
                    if tok.lexeme.contains('.') {
                        if let Ok(f) = tok.lexeme.parse::<f64>() {
                            return Expr::Float(f);
                        }
                    }
                    if let Ok(n) = tok.lexeme.parse::<i64>() {
                        return Expr::Int(n);
                    }
                }
            }
        }
        
        // Check for string literal - use the TokenKind to get the ACTUAL string value
        // Don't use trim_matches on lexeme as that breaks for strings containing quote chars
        if let TokenKind::String(s) = &tok.kind {
            return Expr::String(s.clone());
        }
        
        // Fallback for lexeme-based check (shouldn't be needed, but keep for safety)
        if !tok.lexeme.is_empty() {
            if let Some(first_char) = tok.lexeme.chars().next() {
                if first_char == '"' || first_char == '\'' {
                    // Strip exactly one quote from each end (not trim_matches which strips ALL!)
                    let inner = &tok.lexeme[1..tok.lexeme.len().saturating_sub(1)];
                    return Expr::String(inner.to_string());
                }
            }
        }
        
        // Check for bool/keywords
        if tok.lexeme == "true" {
            return Expr::Bool(true);
        }
        if tok.lexeme == "false" {
            return Expr::Bool(false);
        }
        if tok.lexeme == "none" || tok.lexeme == "None" {
            return Expr::None;
        }
        if tok.lexeme == "self" {
            return Expr::Ident("self".to_string());
        }
        
        // Check for struct/enum construction
        if tok.is_ident() {
            let name = tok.lexeme.clone();
            
            // Enum variant / Static method: Name::Variant or Type::method
            // Note: lexer produces :: as a single ColonColon token
            if self.peek().lexeme == "::" {
                self.advance(); // consume '::'
                let variant = self.advance().lexeme;
                
                // Check for variant with data / method call
                if self.peek().lexeme == "(" {
                    self.advance(); // consume '('
                    let args = self.parse_args();
                    self.advance(); // consume ')'
                    return Expr::EnumVariant(name, variant, args);
                }
                
                return Expr::EnumVariant(name, variant, vec![]);
            }
            
            // Struct literal: Name { field: value, ... }
            if self.peek().lexeme == "{" {
                self.advance(); // consume '{'
                let mut fields = Vec::new();
                let max_fields = 1000;
                
                while !self.is_eof() && fields.len() < max_fields {
                    // Skip newlines and indentation tokens inside braces
                    while matches!(self.peek().kind, TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent) {
                        self.advance();
                    }
                    
                    if self.peek().lexeme == "}" {
                        break;
                    }
                    
                    let field_name = self.advance().lexeme;
                    self.expect(":");
                    let field_value = self.parse_expr();
                    fields.push(FieldInit { name: field_name, value: field_value });
                    
                    if self.peek().lexeme == "," {
                        self.advance();
                    }
                }
                
                if !self.is_eof() && self.peek().lexeme == "}" {
                    self.advance(); // consume '}'
                }
                return Expr::Struct(name, fields);
            }
            
            return Expr::Ident(name);
        }
        
        // Fallback - this is an error!
        panic!("Unexpected token in expression: '{}' at {}:{}", tok.lexeme, tok.line, tok.column);
    }
    
    // =========================================================================
    // Struct/Enum/Impl parsing
    // =========================================================================
    
    fn parse_struct_def(&mut self, is_pub: bool) -> StructDef {
        self.expect_keyword("struct");
        let name = self.advance().lexeme;
        self.expect(":");
        self.skip_newlines();
        
        let mut fields = Vec::new();
        let has_indent = if matches!(self.peek().kind, TokenKind::Indent) {
            self.advance(); true
        } else { false };
        
        while !self.is_eof() {
            let mut tok = self.peek();
            
            if has_indent && matches!(tok.kind, TokenKind::Dedent) {
                self.advance();
                break;
            }
            
            // Stop at new top-level items if NOT indented
            // Note: We removed 'pub' from this check because 'pub' might be a field visibility modifier
            if !has_indent {
                if tok.is_keyword("fn") || tok.is_keyword("struct") 
                   || tok.is_keyword("enum") || tok.is_keyword("impl") || tok.is_keyword("use") {
                    break;
                }
            }
            
            if matches!(tok.kind, TokenKind::Newline) {
                self.advance();
                continue;
            }
            
            // Handle 'pub' field visibility
            if tok.is_keyword("pub") {
                self.advance(); // Consume 'pub'
                tok = self.peek(); // CRITICAL: Update tok to look at the field name!
            }
            
            if tok.is_ident() {
                let field_name = self.advance().lexeme;
                self.expect(":");
                let field_type = self.parse_type();
                fields.push(Field { name: field_name, ty: field_type });
            } else {
                // If we hit something else, we are done with fields
                break;
            }
        }
        
        StructDef { name, fields, is_pub }
    }
    
    fn parse_enum_def(&mut self) -> EnumDef {
        self.expect_keyword("enum");
        let name = self.advance().lexeme;
        self.expect(":");
        self.skip_newlines();
        
        let mut variants = Vec::new();
        let has_indent = if matches!(self.peek().kind, TokenKind::Indent) {
            self.advance(); true
        } else { false };
        
        while !self.is_eof() {
            let tok = self.peek();
            
            if has_indent && matches!(tok.kind, TokenKind::Dedent) {
                self.advance();
                break;
            }
            
            if !has_indent {
                // Stop at new top-level items
                if tok.is_keyword("fn") || tok.is_keyword("pub") || tok.is_keyword("struct") 
                   || tok.is_keyword("enum") || tok.is_keyword("impl") || tok.is_keyword("use") {
                    break;
                }
            }
            
            if matches!(tok.kind, TokenKind::Newline) {
                self.advance();
                continue;
            }
            
            // Parse variant
            if tok.is_ident() {
                let variant_name = self.advance().lexeme;
                let mut variant_fields = Vec::new();
                
                // Check for variant data
                if self.peek().lexeme == "(" {
                    self.advance(); // consume '('
                    while self.peek().lexeme != ")" {
                        variant_fields.push(self.parse_type());
                        if self.peek().lexeme == "," {
                            self.advance();
                        }
                    }
                    self.advance(); // consume ')'
                }
                
                variants.push(Variant { name: variant_name, fields: variant_fields });
            } else {
                break;
            }
        }
        
        EnumDef { name, variants }
    }
    
    fn parse_impl_def(&mut self) -> ImplDef {
        self.expect_keyword("impl");
        let target = self.advance().lexeme;
        self.expect(":");
        self.skip_newlines();
        
        let mut methods = Vec::new();
        
        let has_indent = if matches!(self.peek().kind, TokenKind::Indent) {
            self.advance(); true
        } else { false };
        
        while !self.is_eof() {
            let tok = self.peek();
            
            if has_indent && matches!(tok.kind, TokenKind::Dedent) {
                self.advance();
                break;
            }
            
            if !has_indent {
                 // Stop at new top-level items
                if tok.is_keyword("struct") || tok.is_keyword("enum") 
                   || tok.is_keyword("impl") || tok.is_keyword("use") {
                    break;
                }
                // Stop if fn column 1 (simple heuristic)
                if (tok.is_keyword("fn") || tok.is_keyword("pub")) && tok.column == 1 {
                    break;
                }
            }
            
            if matches!(tok.kind, TokenKind::Newline) {
                self.advance();
                continue;
            }
            
            // Parse method
            if tok.is_keyword("pub") {
                self.advance();
                if self.peek().is_keyword("fn") {
                    let method = self.parse_fn_def(true, false);
                    methods.push(method);
                } else {
                    break;
                }
            } else if tok.is_keyword("fn") {
                let method = self.parse_fn_def(false, false);
                methods.push(method);
            } else {
                break;
            }
        }
        
        ImplDef { target, methods }
    }
    
    fn parse_use(&mut self) -> String {
        self.expect_keyword("use");
        let path = self.advance().lexeme;
        path
    }

    fn parse_extern(&mut self) -> ExternFnDef {
        self.expect_keyword("extern");
        self.expect_keyword("fn");
        
        let name = self.advance().lexeme;
        
        self.expect("(");
        let params = self.parse_params();
        self.expect(")");
        
        let mut return_type = None;
        if self.peek().lexeme == "->" {
            self.advance();
            return_type = Some(self.parse_type());
        }
        
        ExternFnDef {
            name,
            params,
            return_type,
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_simple_function() {
        let source = "fn main(): let x = 42";
        let mut lexer = Lexer::new(source.to_string());
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program();
        assert_eq!(program.items.len(), 1);
    }
}
