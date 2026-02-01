//! KORE Parser - Python-style indentation with Rust semantics

use crate::lexer::{Lexer, Token, TokenKind};
use crate::ast::*;
use crate::span::Span;
use crate::effects::Effect;
use crate::error::{KoreError, KoreResult};

pub struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse(&mut self) -> KoreResult<Program> {
        let mut items = Vec::new();
        let mut top_level_stmts = Vec::new();
        let start = self.current_span();
        
        while !self.at_end() {
            self.skip_newlines();
            if self.at_end() { break; }
            
            match self.peek_kind() {
                TokenKind::Pub | 
                TokenKind::Fn | 
                TokenKind::AsyncKw |
                TokenKind::Component | 
                TokenKind::Shader | 
                TokenKind::Struct | 
                TokenKind::Enum | 
                TokenKind::Actor | 
                TokenKind::Const | 
                TokenKind::Comptime |
                TokenKind::Macro |
                TokenKind::Use |
                TokenKind::Impl |
                TokenKind::Test => {
                    items.push(self.parse_item()?);
                }
                _ => {
                    top_level_stmts.push(self.parse_stmt()?);
                }
            }
        }
        
        if !top_level_stmts.is_empty() {
            let main_fn = Item::Function(Function {
                name: "main".to_string(),
                generics: vec![],
                params: vec![],
                return_type: None,
                effects: vec![],
                body: Block { stmts: top_level_stmts, span: start.merge(self.current_span()) },
                visibility: Visibility::Public,
                span: start.merge(self.current_span()),
            });
            items.push(main_fn);
        }
        
        let end = self.current_span();
        Ok(Program { items, span: start.merge(end) })
    }

    fn parse_item(&mut self) -> KoreResult<Item> {
        let vis = self.parse_visibility();
        
        match self.peek_kind() {
            TokenKind::Fn => self.parse_function(vis),
            TokenKind::AsyncKw => self.parse_async_function(vis),
            TokenKind::Component => self.parse_component(vis),
            TokenKind::Shader => self.parse_shader(),
            TokenKind::Struct => self.parse_struct(vis),
            TokenKind::Enum => self.parse_enum(vis),
            TokenKind::Actor => self.parse_actor(),
            TokenKind::Const => self.parse_const(vis),
            TokenKind::Comptime => self.parse_comptime_block(),
            TokenKind::Macro => self.parse_macro(),
            TokenKind::Test => self.parse_test(),
            TokenKind::Use => self.parse_use(),
            TokenKind::Impl => self.parse_impl(),
            _ => Err(KoreError::parser("Expected item", self.current_span())),
        }
    }

    fn parse_impl(&mut self) -> KoreResult<Item> {
        let start = self.current_span();
        self.expect(TokenKind::Impl)?;
        
        // Parse impl-level generics: impl<T>
        let generics = self.parse_generics()?;
        
        // Parse target type: Option<T>
        // Note: Currently assumes "impl Type". To support "impl Trait for Type", we'd check for 'for'.
        let target_type = self.parse_type()?;
        
        self.expect(TokenKind::Colon)?;
        self.skip_newlines();
        self.expect(TokenKind::Indent)?;
        
        let mut methods = Vec::new();
        
        while !self.check(TokenKind::Dedent) && !self.at_end() {
            self.skip_newlines();
            if self.check(TokenKind::Dedent) { break; }
            
            let vis = self.parse_visibility();
            if self.check(TokenKind::Fn) {
                if let Item::Function(f) = self.parse_function(vis)? {
                    methods.push(f);
                }
            } else {
                return Err(KoreError::parser("Expected fn in impl block", self.current_span()));
            }
            self.skip_newlines();
        }
        
        if self.check(TokenKind::Dedent) {
            self.advance();
        }
        
        Ok(Item::Impl(Impl {
            generics,
            trait_name: None,
            target_type,
            methods,
            span: start.merge(self.current_span()),
        }))
    }

    fn parse_use(&mut self) -> KoreResult<Item> {
        let start = self.current_span();
        self.expect(TokenKind::Use)?;
        
        let mut path = Vec::new();
        path.push(self.parse_ident()?);
        
        // Parse path: use foo::bar::baz OR use foo/bar/baz
        while self.check(TokenKind::ColonColon) || self.check(TokenKind::Slash) {
            self.advance();
            
            // Check for glob: use foo::*
            if self.check(TokenKind::Star) {
                self.advance();
                return Ok(Item::Use(Use { 
                    path, 
                    alias: None, 
                    glob: true, 
                    span: start.merge(self.current_span()) 
                }));
            }
            
            path.push(self.parse_ident()?);
        }
        
        // Check for alias: use foo::bar as baz
        let alias = if self.check(TokenKind::As) {
            self.advance();
            Some(self.parse_ident()?)
        } else {
            None
        };
        
        Ok(Item::Use(Use { 
            path, 
            alias, 
            glob: false, 
            span: start.merge(self.current_span()) 
        }))
    }

    fn parse_test(&mut self) -> KoreResult<Item> {
        let start = self.current_span();
        self.expect(TokenKind::Test)?;
        // Tests can have a string name or identifier
        let name = if let TokenKind::String(s) = self.peek_kind() {
            self.advance();
            s
        } else {
            self.parse_ident()?
        };
        
        self.expect(TokenKind::Colon)?;
        let body = self.parse_block()?;
        Ok(Item::Test(TestDef { name, body, span: start.merge(self.current_span()) }))
    }

    fn parse_macro(&mut self) -> KoreResult<Item> {
        let start = self.current_span();
        self.expect(TokenKind::Macro)?;
        let name = self.parse_ident()?;
        self.expect(TokenKind::Not)?; // macro name!
        self.expect(TokenKind::LParen)?;
        
        let mut params = Vec::new();
        while !self.check(TokenKind::RParen) {
            let p_name = self.parse_ident()?;
            self.expect(TokenKind::Colon)?;
            let kind_name = self.parse_ident()?;
            let kind = match kind_name.as_str() {
                "expr" => MacroParamKind::Expr,
                "type" => MacroParamKind::Type,
                "ident" => MacroParamKind::Ident,
                "block" => MacroParamKind::Block,
                "token" => MacroParamKind::Token,
                _ => return Err(KoreError::parser("Unknown macro param kind", self.current_span())),
            };
            params.push(MacroParam { name: p_name, kind, span: self.current_span() });
            
            if !self.check(TokenKind::RParen) {
                self.expect(TokenKind::Comma)?;
            }
        }
        self.expect(TokenKind::RParen)?;
        self.expect(TokenKind::Colon)?;
        
        let body = self.parse_block()?;
        
        Ok(Item::Macro(MacroDef {
            name,
            params,
            body: MacroBody::Block(body),
            span: start.merge(self.current_span()),
        }))
    }

    fn parse_function(&mut self, vis: Visibility) -> KoreResult<Item> {
        let start = self.current_span();
        self.expect(TokenKind::Fn)?;
        let name = self.parse_ident()?;
        
        // Parse generics: <T, U: Bound>
        let generics = self.parse_generics()?;
        
        self.expect(TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(TokenKind::RParen)?;
        
        let return_type = if self.check(TokenKind::Arrow) {
            self.advance();
            Some(self.parse_type()?)
        } else { None };
        
        let effects = self.parse_effects()?;
        self.expect(TokenKind::Colon)?;
        let body = self.parse_block()?;
        let body_span = body.span;
        
        Ok(Item::Function(Function {
            name, generics, params, return_type, effects, body, visibility: vis,
            span: start.merge(body_span),
        }))
    }

    fn parse_async_function(&mut self, vis: Visibility) -> KoreResult<Item> {
        let start = self.current_span();
        self.expect(TokenKind::AsyncKw)?; // consume 'async'
        self.expect(TokenKind::Fn)?;     // consume 'fn'
        let name = self.parse_ident()?;
        
        // Parse generics: <T, U: Bound>
        let generics = self.parse_generics()?;
        
        self.expect(TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect(TokenKind::RParen)?;
        
        let return_type = if self.check(TokenKind::Arrow) {
            self.advance();
            Some(self.parse_type()?)
        } else { None };
        
        // Parse other effects, then add Async
        let mut effects = self.parse_effects()?;
        effects.push(crate::effects::Effect::Async);
        
        self.expect(TokenKind::Colon)?;
        let body = self.parse_block()?;
        let body_span = body.span;
        
        Ok(Item::Function(Function {
            name, generics, params, return_type, effects, body, visibility: vis,
            span: start.merge(body_span),
        }))
    }
    fn parse_component(&mut self, vis: Visibility) -> KoreResult<Item> {
        let start = self.current_span();
        self.expect(TokenKind::Component)?;
        let name = self.parse_ident()?;
        self.expect(TokenKind::LParen)?;
        let props = self.parse_params()?;
        self.expect(TokenKind::RParen)?;
        let effects = self.parse_effects()?;
        self.expect(TokenKind::Colon)?;
        
        self.skip_newlines();
        self.expect(TokenKind::Indent)?;
        
        let mut state = Vec::new();
        let mut methods = Vec::new();
        let mut body = None;
        
        // Parse component body items
        while !self.check(TokenKind::Dedent) && !self.at_end() {
            self.skip_newlines();
            if self.check(TokenKind::Dedent) { break; }
            
            if self.check(TokenKind::Fn) {
                // Parse method
                if let Item::Function(f) = self.parse_function(Visibility::Private)? {
                    methods.push(f);
                }
            } else if let TokenKind::Ident(ref s) = self.peek_kind() {
                if s == "state" {
                    self.advance();
                    let name = self.parse_ident()?;
                    self.expect(TokenKind::Colon)?;
                    let ty = self.parse_type()?;
                    self.expect(TokenKind::Eq)?;
                    let initial = self.parse_expr()?;
                    state.push(StateDecl { name, ty, initial, weak: false, span: self.current_span() });
                } else if s == "weak" {
                     self.advance();
                     if self.check(TokenKind::Ident("state".to_string())) { // Check specifically for state
                         // "weak state name: Type = ..."
                         self.advance();
                         let name = self.parse_ident()?;
                         self.expect(TokenKind::Colon)?;
                         let ty = self.parse_type()?;
                         self.expect(TokenKind::Eq)?;
                         let initial = self.parse_expr()?;
                         state.push(StateDecl { name, ty, initial, weak: true, span: self.current_span() });
                     } else {
                         return Err(KoreError::parser("Expected 'state' after 'weak' in component", self.current_span()));
                     }
                } else if s == "render" {
                     self.advance();
                     if self.check(TokenKind::LBrace) {
                         // render { jsx }
                         self.advance();
                         self.skip_newlines();
                         body = Some(self.parse_jsx_element()?);
                         self.skip_newlines();
                         self.expect(TokenKind::RBrace)?;
                     } else if self.check(TokenKind::Colon) {
                         // render:
                         //    jsx
                         self.advance();
                         self.skip_newlines();
                         self.expect(TokenKind::Indent)?;
                         self.skip_newlines();
                         body = Some(self.parse_jsx_element()?);
                         self.skip_newlines();
                         self.expect(TokenKind::Dedent)?;
                     } else {
                         // render <jsx>
                         body = Some(self.parse_jsx_element()?);
                     }
                } else {
                    return Err(KoreError::parser(format!("Unexpected identifier in component: {}", s), self.current_span()));
                }
            } else if self.check(TokenKind::Lt) {
                // Direct JSX element (implicit render)
                body = Some(self.parse_jsx_element()?);
            } else {
                return Err(KoreError::parser(format!("Unexpected token in component: {:?}", self.peek_kind()), self.current_span()));
            }
            self.skip_newlines();
        }
        
        if self.check(TokenKind::Dedent) { self.advance(); }
        
        let body = body.ok_or_else(|| KoreError::parser("Component must have a render body (JSX element)", self.current_span()))?;
        
        Ok(Item::Component(Component {
            name, props, state, methods, effects, body, visibility: vis,
            span: start.merge(self.current_span()),
        }))
    }

    fn parse_shader(&mut self) -> KoreResult<Item> {
        let start = self.current_span();
        self.expect(TokenKind::Shader)?;
        
        let stage = if self.check(TokenKind::Vertex) {
            self.advance(); ShaderStage::Vertex
        } else if self.check(TokenKind::Fragment) {
            self.advance(); ShaderStage::Fragment
        } else if let TokenKind::Ident(ref s) = self.peek_kind() {
            if s == "compute" {
                self.advance(); ShaderStage::Compute
            } else {
                ShaderStage::Fragment // Default
            }
        } else {
            ShaderStage::Fragment // Default
        };

        let name = self.parse_ident()?;
        self.expect(TokenKind::LParen)?;
        let inputs = self.parse_params()?;
        self.expect(TokenKind::RParen)?;
        self.expect(TokenKind::Arrow)?;
        let outputs = self.parse_type()?;
        self.expect(TokenKind::Colon)?;
        
        // Manual block parsing to support uniforms
        self.skip_newlines();
        let block_start = self.current_span();
        self.expect(TokenKind::Indent)?;

        let mut uniforms = Vec::new();
        let mut stmts = Vec::new();

        while !self.check(TokenKind::Dedent) && !self.at_end() {
            self.skip_newlines();
            if self.check(TokenKind::Dedent) { break; }

            // Check for "uniform" identifier
            let is_uniform = if let TokenKind::Ident(ref s) = self.peek_kind() {
                s == "uniform"
            } else {
                false
            };

            if is_uniform {
                self.advance(); // consume "uniform"
                let u_name = self.parse_ident()?;
                self.expect(TokenKind::Colon)?;
                let u_ty = self.parse_type()?;
                self.expect(TokenKind::At)?;
                
                // Parse integer binding
                let binding = if let TokenKind::Int(n) = self.peek_kind() {
                    self.advance();
                    n as u32
                } else {
                    return Err(KoreError::parser("Expected integer binding", self.current_span()));
                };

                uniforms.push(Uniform { name: u_name, ty: u_ty, binding, span: self.current_span() });
            } else {
                stmts.push(self.parse_stmt()?);
            }
            self.skip_newlines();
        }

        if self.check(TokenKind::Dedent) { self.advance(); }
        let body = Block { stmts, span: block_start.merge(self.current_span()) };
        let body_span = body.span;
        
        Ok(Item::Shader(Shader {
            name, stage, inputs, outputs, uniforms, body,
            span: start.merge(body_span),
        }))
    }

    fn parse_struct(&mut self, vis: Visibility) -> KoreResult<Item> {
        let start = self.current_span();
        self.expect(TokenKind::Struct)?;
        let name = self.parse_ident()?;
        
        // Parse generics: struct Box<T>:
        let generics = self.parse_generics()?;
        
        self.expect(TokenKind::Colon)?;
        self.skip_newlines();
        self.expect(TokenKind::Indent)?;
        
        let mut fields = Vec::new();
        while !self.check(TokenKind::Dedent) && !self.at_end() {
            self.skip_newlines();
            if self.check(TokenKind::Dedent) { break; }
            
            // Check for weak
            let weak = if let TokenKind::Ident(s) = self.peek_kind() {
                if s == "weak" {
                    self.advance();
                    true
                } else { false }
            } else { false };
            
            let fname = self.parse_ident()?;
            self.expect(TokenKind::Colon)?;
            let ty = self.parse_type()?;
            fields.push(Field { name: fname, ty, visibility: Visibility::Public, default: None, weak, span: self.current_span() });
            self.skip_newlines();
        }
        if self.check(TokenKind::Dedent) { self.advance(); }
        
        Ok(Item::Struct(Struct { name, generics, fields, visibility: vis, span: start.merge(self.current_span()) }))
    }

    fn parse_enum(&mut self, vis: Visibility) -> KoreResult<Item> {
        let start = self.current_span();
        self.expect(TokenKind::Enum)?;
        let name = self.parse_ident()?;
        
        // Parse generics: enum Option<T>:
        let generics = self.parse_generics()?;
        
        self.expect(TokenKind::Colon)?;
        self.skip_newlines();
        self.expect(TokenKind::Indent)?;
        
        let mut variants = Vec::new();
        while !self.check(TokenKind::Dedent) && !self.at_end() {
            self.skip_newlines();
            if self.check(TokenKind::Dedent) { break; }
            let start_span = self.current_span();
            let vname = self.parse_ident()?;
            
            let fields = if self.check(TokenKind::LParen) {
                self.advance(); // consume (
                let mut types = Vec::new();
                while !self.check(TokenKind::RParen) && !self.at_end() {
                    types.push(self.parse_type()?);
                    if !self.check(TokenKind::RParen) {
                        self.expect(TokenKind::Comma)?;
                    }
                }
                self.expect(TokenKind::RParen)?;
                VariantFields::Tuple(types)
            } else if self.check(TokenKind::LBrace) {
                self.advance(); // consume {
                self.skip_newlines();
                let indented = if self.check(TokenKind::Indent) { self.advance(); true } else { false };
                
                let mut fields = Vec::new();
                while !self.check(TokenKind::RBrace) && !self.at_end() {
                    if indented && self.check(TokenKind::Dedent) { break; }
                    if !indented && self.check(TokenKind::RBrace) { break; }
                    
                    self.skip_newlines();
                    if self.check(TokenKind::RBrace) || (indented && self.check(TokenKind::Dedent)) { break; }
                    
                    let fname = self.parse_ident()?;
                    self.expect(TokenKind::Colon)?;
                    let ty = self.parse_type()?;
                    
                    fields.push(Field { 
                        name: fname, 
                        ty, 
                        visibility: Visibility::Public, 
                        default: None, 
                        weak: false, 
                        span: self.current_span() 
                    });
                    
                    if !self.check(TokenKind::RBrace) {
                         if self.check(TokenKind::Comma) { self.advance(); }
                    }
                    self.skip_newlines();
                }
                
                if indented { self.expect(TokenKind::Dedent)?; }
                self.expect(TokenKind::RBrace)?;
                VariantFields::Struct(fields)
            } else {
                VariantFields::Unit
            };
            
            // If the next token is a newline/dedent, previous token end is the end of the variant
            // For now, using current_span (next token) is consistent with previous code's behavior for Unit variants
            // but for Tuple/Struct it's better to span the whole thing.
            // Let's use start_span (ident) merged with current_span (after fields).
            let span = if matches!(fields, VariantFields::Unit) {
                 // If Unit, current_span is the one after ident. 
                 // If we used start_span, it would be the ident.
                 // Let's just use start_span for Unit to correct the "bug" of using next token.
                 // But wait, start_span is the Ident span.
                 start_span
            } else {
                 // For fields, we consumed ) or }. current_span is the one after that.
                 // We want to merge start_span with the end of the fields.
                 // But current_span() points to the *next* token.
                 // We can use start_span for the start.
                 // For the end, it's a bit tricky without keeping track of the last consumed token.
                 // We'll just use start_span.merge(self.current_span()) which covers [Ident ... NextToken].
                 // That's acceptable.
                 start_span.merge(self.current_span())
            };

            variants.push(Variant { name: vname, fields, span });
            self.skip_newlines();
        }
        if self.check(TokenKind::Dedent) { self.advance(); }
        
        Ok(Item::Enum(Enum { name, generics, variants, visibility: vis, span: start.merge(self.current_span()) }))
    }

    fn parse_actor(&mut self) -> KoreResult<Item> {
        let start = self.current_span();
        self.expect(TokenKind::Actor)?;
        let name = self.parse_ident()?;
        self.expect(TokenKind::Colon)?;
        
        self.skip_newlines();
        self.expect(TokenKind::Indent)?;
        
        let mut state = Vec::new();
        let mut handlers = Vec::new();
        
        while !self.check(TokenKind::Dedent) && !self.at_end() {
            self.skip_newlines();
            if self.check(TokenKind::Dedent) { break; }
            
            // Check for "state" or "on"
            if let TokenKind::Ident(s) = self.peek_kind() {
                if s == "state" {
                    self.advance();
                    let name = self.parse_ident()?;
                    self.expect(TokenKind::Colon)?;
                    let ty = self.parse_type()?;
                    self.expect(TokenKind::Eq)?;
                    let initial = self.parse_expr()?;
                    state.push(StateDecl { name, ty, initial, weak: false, span: self.current_span() });
                } else if s == "weak" {
                    self.advance();
                    if self.check(TokenKind::Ident("state".to_string())) {
                        self.advance();
                        let name = self.parse_ident()?;
                        self.expect(TokenKind::Colon)?;
                        let ty = self.parse_type()?;
                        self.expect(TokenKind::Eq)?;
                        let initial = self.parse_expr()?;
                        state.push(StateDecl { name, ty, initial, weak: true, span: self.current_span() });
                    } else {
                         return Err(KoreError::parser("Expected 'state' after 'weak' in actor", self.current_span()));
                    }
                } else if s == "on" {
                    self.advance();
                    let message_type = self.parse_ident()?;
                    self.expect(TokenKind::LParen)?;
                    let params = self.parse_params()?;
                    self.expect(TokenKind::RParen)?;
                    self.expect(TokenKind::Colon)?;
                    let body = self.parse_block()?;
                    handlers.push(MessageHandler { message_type, params, body, span: self.current_span() });
                } else {
                     return Err(KoreError::parser(format!("Unexpected item in actor: {}", s), self.current_span()));
                }
            } else {
                 return Err(KoreError::parser("Expected 'state' or 'on' in actor definition", self.current_span()));
            }
            
            self.skip_newlines();
        }
        if self.check(TokenKind::Dedent) { self.advance(); }
        
        let span = start.merge(self.current_span());
        Ok(Item::Actor(Actor { name, state, handlers, span }))
    }

    fn parse_const(&mut self, vis: Visibility) -> KoreResult<Item> {
        let start = self.current_span();
        self.expect(TokenKind::Const)?;
        let name = self.parse_ident()?;
        self.expect(TokenKind::Colon)?;
        let ty = self.parse_type()?;
        self.expect(TokenKind::Eq)?;
        let value = self.parse_expr()?;
        Ok(Item::Const(Const { name, ty, value, visibility: vis, span: start.merge(self.current_span()) }))
    }

    fn parse_comptime_block(&mut self) -> KoreResult<Item> {
        let start = self.current_span();
        self.expect(TokenKind::Comptime)?;
        self.expect(TokenKind::Colon)?;
        let body = self.parse_block()?;
        Ok(Item::Comptime(ComptimeBlock { body, span: start.merge(self.current_span()) }))
    }

    fn parse_params(&mut self) -> KoreResult<Vec<Param>> {
        let mut params = Vec::new();
        self.skip_newlines();
        while !self.check(TokenKind::RParen) && !self.at_end() {
            let mutable = if self.check(TokenKind::Mut) {
                self.advance();
                true
            } else {
                false
            };

            let name = self.parse_ident()?;
            let ty = if self.check(TokenKind::Colon) {
                self.advance();
                self.parse_type()?
            } else {
                Type::Infer(self.current_span())
            };
            params.push(Param { name, ty, mutable, default: None, span: self.current_span() });
            
            self.skip_newlines();
            if !self.check(TokenKind::RParen) { 
                self.expect(TokenKind::Comma)?; 
                self.skip_newlines();
            }
        }
        Ok(params)
    }

    /// Parse generic type parameters: <T, U: Bound, V>
    fn parse_generics(&mut self) -> KoreResult<Vec<Generic>> {
        let mut generics = Vec::new();
        
        // Check for opening <
        if !self.check(TokenKind::Lt) {
            return Ok(generics);
        }
        self.advance(); // consume <
        
        while !self.check(TokenKind::Gt) && !self.at_end() {
            let start = self.current_span();
            let name = self.parse_ident()?;
            
            // Parse optional bounds: T: Bound1 + Bound2
            let mut bounds = Vec::new();
            if self.check(TokenKind::Colon) {
                self.advance(); // consume :
                loop {
                    let bound_name = self.parse_ident()?;
                    bounds.push(TypeBound { trait_name: bound_name, span: self.current_span() });
                    if !self.check(TokenKind::Plus) { break; }
                    self.advance(); // consume +
                }
            }
            
            generics.push(Generic { name, bounds, span: start.merge(self.current_span()) });
            
            if !self.check(TokenKind::Gt) {
                self.expect(TokenKind::Comma)?;
            }
        }
        
        self.expect(TokenKind::Gt)?; // consume >
        Ok(generics)
    }

    fn parse_effects(&mut self) -> KoreResult<Vec<Effect>> {
        let mut effects = Vec::new();
        if self.check(TokenKind::With) {
            self.advance();
            loop {
                // Effects are keywords, not identifiers
                let effect = match self.peek_kind() {
                    TokenKind::Pure => { self.advance(); Some(Effect::Pure) }
                    TokenKind::Io => { self.advance(); Some(Effect::IO) }
                    TokenKind::Async => { self.advance(); Some(Effect::Async) }
                    TokenKind::Gpu => { self.advance(); Some(Effect::GPU) }
                    TokenKind::Reactive => { self.advance(); Some(Effect::Reactive) }
                    TokenKind::Unsafe => { self.advance(); Some(Effect::Unsafe) }
                    TokenKind::Ident(ref s) => {
                        let e = Effect::from_str(s);
                        self.advance();
                        e
                    }
                    _ => None,
                };
                if let Some(e) = effect {
                    effects.push(e);
                }
                if !self.check(TokenKind::Comma) { break; }
                self.advance();
            }
        }
        Ok(effects)
    }

    fn parse_type(&mut self) -> KoreResult<Type> {
        let span = self.current_span();
        
        // Handle tuple types: (A, B) or unit type: ()
        if self.check(TokenKind::LParen) {
            self.advance(); // consume (
            
            // Check for unit type ()
            if self.check(TokenKind::RParen) {
                self.advance(); // consume )
                return Ok(Type::Unit(span.merge(self.current_span())));
            }
            
            // Parse tuple elements
            let mut elements = Vec::new();
            elements.push(self.parse_type()?);
            
            while self.check(TokenKind::Comma) {
                self.advance(); // consume ,
                if self.check(TokenKind::RParen) { break; } // trailing comma
                elements.push(self.parse_type()?);
            }
            
            self.expect(TokenKind::RParen)?;
            return Ok(Type::Tuple(elements, span.merge(self.current_span())));
        }
        
        // Handle impl Trait: impl Future, impl Iterator<Item = T>
        if self.check(TokenKind::Impl) {
            self.advance(); // consume impl
            let trait_name = self.parse_ident()?;
            
            // Parse generic arguments if present
            let mut generics = Vec::new();
            if self.check(TokenKind::Lt) {
                self.advance(); // consume <
                while !self.check(TokenKind::Gt) && !self.at_end() {
                    generics.push(self.parse_type()?);
                    if !self.check(TokenKind::Gt) {
                        self.expect(TokenKind::Comma)?;
                    }
                }
                self.expect(TokenKind::Gt)?;
            }
            
            return Ok(Type::Impl {
                trait_name,
                generics,
                span: span.merge(self.current_span()),
            });
        }
        
        // Handle function types: fn(T, U) -> R
        if self.check(TokenKind::Fn) {
            self.advance(); // consume fn
            self.expect(TokenKind::LParen)?;
            
            // Parse parameter types
            let mut params = Vec::new();
            while !self.check(TokenKind::RParen) && !self.at_end() {
                params.push(self.parse_type()?);
                if !self.check(TokenKind::RParen) {
                    self.expect(TokenKind::Comma)?;
                }
            }
            self.expect(TokenKind::RParen)?;
            
            // Parse return type (optional)
            let return_type = if self.check(TokenKind::Arrow) {
                self.advance(); // consume ->
                Box::new(self.parse_type()?)
            } else {
                Box::new(Type::Unit(span))
            };
            
            return Ok(Type::Function {
                params,
                return_type,
                effects: vec![],
                span: span.merge(self.current_span()),
            });
        }
        
        let mut name = self.parse_ident()?;
        
        // Support Module::Type syntax
        while self.check(TokenKind::ColonColon) {
            self.advance(); // consume ::
            let part = self.parse_ident()?;
            name.push_str("::");
            name.push_str(&part);
        }
        
        // Parse generic type arguments: Type<T, U>
        let mut type_args = Vec::new();
        if self.check(TokenKind::Lt) {
            self.advance(); // consume <
            while !self.check(TokenKind::Gt) && !self.at_end() {
                type_args.push(self.parse_type()?);
                if !self.check(TokenKind::Gt) {
                    self.expect(TokenKind::Comma)?;
                }
            }
            self.expect(TokenKind::Gt)?; // consume >
        }
        
        Ok(Type::Named { name, generics: type_args, span })
    }

    fn parse_block(&mut self) -> KoreResult<Block> {
        self.skip_newlines();
        let start = self.current_span();
        self.expect(TokenKind::Indent)?;
        
        let mut stmts = Vec::new();
        while !self.check(TokenKind::Dedent) && !self.at_end() {
            self.skip_newlines();
            if self.check(TokenKind::Dedent) { break; }
            stmts.push(self.parse_stmt()?);
            self.skip_newlines();
        }
        if self.check(TokenKind::Dedent) { self.advance(); }
        
        Ok(Block { stmts, span: start.merge(self.current_span()) })
    }

    fn parse_stmt(&mut self) -> KoreResult<Stmt> {
        match self.peek_kind() {
            TokenKind::Let => self.parse_let(),
            TokenKind::Return => self.parse_return(),
            TokenKind::For => self.parse_for(),
            TokenKind::While => self.parse_while(),
            TokenKind::Loop => self.parse_loop(),
            TokenKind::Break => self.parse_break(),
            TokenKind::Continue => self.parse_continue(),
            _ => Ok(Stmt::Expr(self.parse_expr()?)),
        }
    }

    fn parse_let(&mut self) -> KoreResult<Stmt> {
        let start = self.current_span();
        self.expect(TokenKind::Let)?;
        let pattern = self.parse_pattern()?;
        let ty = if self.check(TokenKind::Colon) { self.advance(); Some(self.parse_type()?) } else { None };
        self.expect(TokenKind::Eq)?;
        let value = Some(self.parse_expr()?);
        Ok(Stmt::Let { pattern, ty, value, span: start.merge(self.current_span()) })
    }

    fn parse_return(&mut self) -> KoreResult<Stmt> {
        let start = self.current_span();
        self.expect(TokenKind::Return)?;
        let value = if !self.check_line_end() { Some(self.parse_expr()?) } else { None };
        Ok(Stmt::Return(value, start.merge(self.current_span())))
    }

    fn parse_for(&mut self) -> KoreResult<Stmt> {
        let start = self.current_span();
        self.expect(TokenKind::For)?;
        let name = self.parse_ident()?;
        self.expect(TokenKind::In)?;
        let iter = self.parse_expr()?;
        self.expect(TokenKind::Colon)?;
        let body = self.parse_block()?;
        Ok(Stmt::For { binding: Pattern::Binding { name, mutable: false, span: start }, iter, body, span: start.merge(self.current_span()) })
    }

    fn parse_while(&mut self) -> KoreResult<Stmt> {
        let start = self.current_span();
        self.expect(TokenKind::While)?;
        let condition = self.parse_expr()?;
        self.expect(TokenKind::Colon)?;
        let body = self.parse_block()?;
        Ok(Stmt::While { condition, body, span: start.merge(self.current_span()) })
    }

    fn parse_loop(&mut self) -> KoreResult<Stmt> {
        let start = self.current_span();
        self.expect(TokenKind::Loop)?;
        self.expect(TokenKind::Colon)?;
        let body = self.parse_block()?;
        Ok(Stmt::Loop { body, span: start.merge(self.current_span()) })
    }

    fn parse_break(&mut self) -> KoreResult<Stmt> {
        let start = self.current_span();
        self.expect(TokenKind::Break)?;
        // Optional value: break expr
        let value = if !self.check_line_end() && !self.check(TokenKind::Dedent) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        Ok(Stmt::Break(value, start.merge(self.current_span())))
    }

    fn parse_continue(&mut self) -> KoreResult<Stmt> {
        let start = self.current_span();
        self.expect(TokenKind::Continue)?;
        Ok(Stmt::Continue(start.merge(self.current_span())))
    }
    fn parse_expr(&mut self) -> KoreResult<Expr> { self.parse_assignment() }

    fn parse_assignment(&mut self) -> KoreResult<Expr> {
        let expr = self.parse_binary(0)?;
        
        if self.check(TokenKind::Eq) {
            self.advance();
            let value = self.parse_assignment()?;
            let span = expr.span().merge(value.span());
            Ok(Expr::Assign { target: Box::new(expr), value: Box::new(value), span })
        } else {
            Ok(expr)
        }
    }

    fn parse_binary(&mut self, min_prec: u8) -> KoreResult<Expr> {
        let mut left = self.parse_unary()?;
        
        while let Some((op, prec)) = self.get_binary_op() {
            if prec < min_prec { break; }
            self.advance();
            let right = self.parse_binary(prec + 1)?;
            let span = left.span().merge(right.span());
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right), span };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> KoreResult<Expr> {
        match self.peek_kind() {
            TokenKind::Minus => { let s = self.current_span(); self.advance(); Ok(Expr::Unary { op: UnaryOp::Neg, operand: Box::new(self.parse_unary()?), span: s }) }
            TokenKind::Not => { let s = self.current_span(); self.advance(); Ok(Expr::Unary { op: UnaryOp::Not, operand: Box::new(self.parse_unary()?), span: s }) }
            TokenKind::Await => {
                let start = self.current_span();
                self.advance();
                let expr = self.parse_unary()?; // Right-associative: await await x
                Ok(Expr::Await(Box::new(expr), start.merge(self.current_span())))
            }
            TokenKind::Send => {
                let start = self.current_span();
                self.advance();
                let expr = self.parse_postfix()?;
                
                if let Expr::Call { callee, args, span } = expr {
                    if let Expr::Field { object, field, span: _ } = *callee {
                        let mut data = Vec::new();
                        for arg in args {
                            if let Some(name) = arg.name {
                                data.push((name, arg.value));
                            } else {
                                return Err(KoreError::parser("Send requires named arguments", arg.span));
                            }
                        }
                        Ok(Expr::SendMsg { target: object, message: field, data, span: start.merge(span) })
                    } else {
                        Err(KoreError::parser("Expected method call after send (e.g., actor.message())", span))
                    }
                } else {
                    Err(KoreError::parser("Expected message call after send", expr.span()))
                }
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> KoreResult<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.peek_kind() {
                TokenKind::LParen => { 
                    self.advance(); 
                    let args = self.parse_call_args()?; 
                    self.expect(TokenKind::RParen)?; 
                    let s = expr.span().merge(self.current_span()); 
                    
                    if let Expr::Field { object, field, span: _ } = expr {
                        expr = Expr::MethodCall { receiver: object, method: field, args, span: s };
                    } else {
                        expr = Expr::Call { callee: Box::new(expr), args, span: s }; 
                    }
                }
                TokenKind::Dot => { self.advance(); let field = self.parse_ident()?; let s = expr.span().merge(self.current_span()); expr = Expr::Field { object: Box::new(expr), field, span: s }; }
            TokenKind::As => {
                self.advance();
                let target = self.parse_type()?;
                let s = expr.span().merge(self.current_span());
                expr = Expr::Cast { value: Box::new(expr), target, span: s };
            }
            TokenKind::LBracket => { self.advance(); let idx = self.parse_expr()?; self.expect(TokenKind::RBracket)?; let s = expr.span().merge(self.current_span()); expr = Expr::Index { object: Box::new(expr), index: Box::new(idx), span: s }; }
            TokenKind::Question => { self.advance(); let s = expr.span().merge(self.current_span()); expr = Expr::Try(Box::new(expr), s); }
            TokenKind::Not => {
                // Macro invocation: ident!(args)
                if let Expr::Ident(name, _) = &expr {
                    self.advance(); // consume '!'
                    self.expect(TokenKind::LParen)?;
                    let args = if !self.check(TokenKind::RParen) {
                        let mut args = Vec::new();
                        args.push(self.parse_expr()?);
                        while self.check(TokenKind::Comma) {
                            self.advance();
                            if self.check(TokenKind::RParen) { break; }
                            args.push(self.parse_expr()?);
                        }
                        args
                    } else {
                        Vec::new()
                    };
                    self.expect(TokenKind::RParen)?;
                    let s = expr.span().merge(self.current_span());
                    expr = Expr::MacroCall { name: name.clone(), args, span: s };
                } else {
                     // Maybe unary not? But we are in postfix. Unary not is handled in parse_unary.
                     // Postfix ! usually means macro or maybe future features (like factorial?).
                     // For now, only support macros on identifiers.
                     return Err(KoreError::parser("Macro invocation only allowed on identifiers", self.current_span()));
                }
            }
            _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> KoreResult<Expr> {
        let span = self.current_span();
        match self.peek_kind() {
            TokenKind::Int(n) => { self.advance(); Ok(Expr::Int(n, span)) }
            TokenKind::Float(n) => { self.advance(); Ok(Expr::Float(n, span)) }
            TokenKind::String(ref s) => { let s = s.clone(); self.advance(); Ok(Expr::String(s, span)) }
            TokenKind::FString(ref s) => {
                let s = s.clone();
                self.advance();
                let mut parts = Vec::new();
                let mut last_idx = 0;
                let mut chars = s.char_indices().peekable();
                
                while let Some((idx, c)) = chars.next() {
                    if c == '{' {
                        if idx > last_idx {
                            parts.push(Expr::String(s[last_idx..idx].to_string(), span));
                        }
                        
                        let expr_start = idx + 1;
                        let mut depth = 1;
                        let mut expr_end = expr_start;
                        
                        while let Some((i, c2)) = chars.next() {
                            if c2 == '{' { depth += 1; }
                            else if c2 == '}' {
                                depth -= 1;
                                if depth == 0 {
                                    expr_end = i;
                                    break;
                                }
                            }
                        }
                        
                        if depth == 0 {
                            let expr_str = &s[expr_start..expr_end];
                            let tokens = Lexer::new(expr_str).tokenize()?;
                            let mut parser = Parser::new(&tokens);
                            let expr = parser.parse_expr()?;
                            parts.push(expr);
                            last_idx = expr_end + 1;
                        } else {
                             return Err(KoreError::parser("Unclosed '{' in f-string", span));
                        }
                    }
                }
                
                if last_idx < s.len() {
                    parts.push(Expr::String(s[last_idx..].to_string(), span));
                }
                
                Ok(Expr::FString(parts, span))
            }
            TokenKind::True => { self.advance(); Ok(Expr::Bool(true, span)) }
            TokenKind::False => { self.advance(); Ok(Expr::Bool(false, span)) }
            TokenKind::None => { self.advance(); Ok(Expr::None(span)) }
            TokenKind::Ident(ref s) => { 
                let name = s.clone(); 
                self.advance();

                if self.check(TokenKind::ColonColon) {
                    self.advance();
                    let variant = self.parse_ident()?;

                    let fields = if self.check(TokenKind::LParen) {
                        self.advance();
                        self.skip_newlines();
                        let mut items = Vec::new();
                        if !self.check(TokenKind::RParen) {
                            items.push(self.parse_expr()?);
                            while self.check(TokenKind::Comma) {
                                self.advance();
                                if self.check(TokenKind::RParen) {
                                    break;
                                }
                                items.push(self.parse_expr()?);
                            }
                        }
                        self.expect(TokenKind::RParen)?;
                        if items.is_empty() {
                            EnumVariantFields::Unit
                        } else {
                            EnumVariantFields::Tuple(items)
                        }
                    } else if self.check(TokenKind::LBrace) {
                        self.advance();
                        let mut fields = Vec::new();

                        self.skip_newlines();
                        let indented = if self.check(TokenKind::Indent) {
                            self.advance();
                            true
                        } else {
                            false
                        };

                        while !self.check(TokenKind::RBrace) && !self.at_end() {
                            if indented && self.check(TokenKind::Dedent) {
                                break;
                            }

                            let field_name = self.parse_ident()?;
                            self.expect(TokenKind::Colon)?;
                            let field_value = self.parse_expr()?;
                            fields.push((field_name, field_value));

                            if !self.check(TokenKind::RBrace) && (!indented || !self.check(TokenKind::Dedent)) {
                                if self.check(TokenKind::Comma) {
                                    self.advance();
                                }
                            }
                            self.skip_newlines();
                        }

                        if indented {
                            self.expect(TokenKind::Dedent)?;
                        }
                        self.expect(TokenKind::RBrace)?;
                        EnumVariantFields::Struct(fields)
                    } else {
                        EnumVariantFields::Unit
                    };

                    return Ok(Expr::EnumVariant {
                        enum_name: name,
                        variant,
                        fields,
                        span: span.merge(self.current_span()),
                    });
                }
                
                // Check if this is a struct literal: Name { field: value, ... }
                if self.check(TokenKind::LBrace) {
                    self.advance(); // consume {
                    let mut fields = Vec::new();
                    
                    self.skip_newlines();
                    let indented = if self.check(TokenKind::Indent) {
                        self.advance();
                        true
                    } else {
                        false
                    };
                    
                    while !self.check(TokenKind::RBrace) && !self.at_end() {
                        if indented && self.check(TokenKind::Dedent) {
                            break;
                        }
                        
                        let field_name = self.parse_ident()?;
                        self.expect(TokenKind::Colon)?;
                        let field_value = self.parse_expr()?;
                        fields.push((field_name, field_value));
                        
                        // Optional comma if not closing
                        if !self.check(TokenKind::RBrace) && (!indented || !self.check(TokenKind::Dedent)) {
                            if self.check(TokenKind::Comma) {
                                self.advance();
                            }
                        }
                        self.skip_newlines();
                    }
                    
                    if indented {
                        self.expect(TokenKind::Dedent)?;
                    }
                    self.expect(TokenKind::RBrace)?;
                    
                    Ok(Expr::Struct { 
                        name, 
                        fields, 
                        span: span.merge(self.current_span()) 
                    })
                } else {
                    Ok(Expr::Ident(name, span))
                }
            }
            TokenKind::SelfLower => { 
                self.advance(); 
                Ok(Expr::Ident("self".to_string(), span)) 
            }
            TokenKind::SelfUpper => { 
                self.advance(); 
                Ok(Expr::Ident("Self".to_string(), span)) 
            }
            TokenKind::LParen => { 
                self.advance(); 
                if self.check(TokenKind::RParen) {
                    self.advance();
                    Ok(Expr::Tuple(vec![], span.merge(self.current_span())))
                } else {
                    let first = self.parse_expr()?;
                    if self.check(TokenKind::Comma) {
                        self.advance();
                        let mut items = vec![first];
                        while !self.check(TokenKind::RParen) {
                            items.push(self.parse_expr()?);
                            if !self.check(TokenKind::RParen) { self.expect(TokenKind::Comma)?; }
                        }
                        self.expect(TokenKind::RParen)?;
                        Ok(Expr::Tuple(items, span.merge(self.current_span())))
                    } else {
                        self.expect(TokenKind::RParen)?;
                        Ok(Expr::Paren(Box::new(first), span.merge(self.current_span())))
                    }
                }
            }
            TokenKind::LBracket => { 
                self.advance();
                self.skip_newlines();
                
                // Check for indent (multi-line array)
                let indented = if self.check(TokenKind::Indent) {
                    self.advance();
                    true
                } else {
                    false
                };
                
                let mut items = vec![];
                while !self.check(TokenKind::RBracket) && !self.at_end() {
                    if indented && self.check(TokenKind::Dedent) {
                        break;
                    }
                    self.skip_newlines();
                    if self.check(TokenKind::RBracket) {
                        break;
                    }
                    items.push(self.parse_expr()?);
                    self.skip_newlines();
                    if !self.check(TokenKind::RBracket) && !self.check(TokenKind::Dedent) { 
                        if self.check(TokenKind::Comma) {
                            self.advance();
                            self.skip_newlines();
                        }
                    }
                }
                
                if indented {
                    if self.check(TokenKind::Dedent) {
                        self.advance();
                    }
                }
                self.skip_newlines();
                self.expect(TokenKind::RBracket)?; 
                Ok(Expr::Array(items, span)) 
            }
            TokenKind::Comptime => {
                self.advance();
                self.expect(TokenKind::Colon)?;
                let body = self.parse_block()?;
                Ok(Expr::Comptime(Box::new(Expr::Block(body, span)), span))
            }
            TokenKind::Pipe => {
                self.advance();
                let mut params = Vec::new();
                while !self.check(TokenKind::Pipe) {
                    let name = self.parse_ident()?;
                    params.push(Param {
                        name,
                        ty: Type::Infer(span),
                        mutable: false,
                        default: None,
                        span,
                    });
                    if !self.check(TokenKind::Pipe) { self.expect(TokenKind::Comma)?; }
                }
                self.expect(TokenKind::Pipe)?;
                let body = self.parse_expr()?;
                Ok(Expr::Lambda { params, return_type: None, body: Box::new(body), span: span.merge(self.current_span()) })
            }
            TokenKind::Match => self.parse_match(),
            TokenKind::Spawn => {
                self.advance();
                let actor = self.parse_ident()?;
                self.expect(TokenKind::LParen)?;
                let args = self.parse_call_args()?;
                self.expect(TokenKind::RParen)?;
                
                let mut init = Vec::new();
                for arg in args {
                    if let Some(name) = arg.name {
                        init.push((name, arg.value));
                    } else {
                         return Err(KoreError::parser("Spawn requires named arguments", arg.span));
                    }
                }
                Ok(Expr::Spawn { actor, init, span: span.merge(self.current_span()) })
            }
            TokenKind::Return => {
                let start = self.current_span();
                self.advance();
                let value = if !self.check_line_end() 
                    && !self.check(TokenKind::Comma) 
                    && !self.check(TokenKind::RParen) 
                    && !self.check(TokenKind::RBrace) 
                    && !self.check(TokenKind::RBracket) 
                {
                    Some(Box::new(self.parse_expr()?))
                } else {
                    None
                };
                Ok(Expr::Return(value, start.merge(self.current_span())))
            }
            TokenKind::If => self.parse_if(),
            TokenKind::Lt => {
                let jsx = self.parse_jsx_element()?;
                Ok(Expr::JSX(jsx, span.merge(self.current_span())))
            }
            // Lambda with fn syntax: fn(x: Int) -> Int: return x * 2
            // or fn(x: Int): return x * 2
            TokenKind::Fn => {
                self.advance(); // consume fn
                self.expect(TokenKind::LParen)?;
                
                // Parse parameters with types
                let mut params = Vec::new();
                while !self.check(TokenKind::RParen) && !self.at_end() {
                    let p_span = self.current_span();
                    let name = self.parse_ident()?;
                    
                    // Parse type annotation
                    let ty = if self.check(TokenKind::Colon) {
                        self.advance();
                        self.parse_type()?
                    } else {
                        Type::Infer(p_span)
                    };
                    
                    params.push(Param {
                        name,
                        ty,
                        mutable: false,
                        default: None,
                        span: p_span,
                    });
                    
                    if !self.check(TokenKind::RParen) {
                        self.expect(TokenKind::Comma)?;
                    }
                }
                self.expect(TokenKind::RParen)?;
                
                // Parse optional return type
                let return_type = if self.check(TokenKind::Arrow) {
                    self.advance();
                    Some(self.parse_type()?)
                } else {
                    None
                };
                
                self.expect(TokenKind::Colon)?;
                
                // Parse body - can be a single expression or a block
                let body = if self.check(TokenKind::Return) {
                    // Single return statement: fn(x): return x * 2
                    self.advance();
                    self.parse_expr()?
                } else if self.check(TokenKind::Indent) || self.check_newline() {
                    // Block body (multi-line lambda)
                    self.skip_newlines();
                    let block = self.parse_block()?;
                    Expr::Block(block, span)
                } else {
                    // Single expression: fn(x): x * 2
                    self.parse_expr()?
                };
                
                Ok(Expr::Lambda { 
                    params, 
                    return_type, 
                    body: Box::new(body), 
                    span: span.merge(self.current_span()) 
                })
            }
            // Control flow as expressions (for use in match arms, etc.)
            TokenKind::Continue => {
                self.advance();
                // Continue as an expression wraps in a block that continues
                Ok(Expr::Continue(span))
            }
            TokenKind::Break => {
                self.advance();
                // Optional break value
                let value = if !self.check_line_end() && !self.check(TokenKind::Dedent) 
                    && !self.check(TokenKind::Comma) && !self.check(TokenKind::RParen) {
                    Some(Box::new(self.parse_expr()?))
                } else {
                    None
                };
                Ok(Expr::Break(value, span))
            }
            _ => Err(KoreError::parser(format!("Unexpected token: {:?}", self.peek_kind()), span)),
        }
    }

    fn parse_match(&mut self) -> KoreResult<Expr> {
        let start = self.current_span();
        self.expect(TokenKind::Match)?;
        let scrutinee = Box::new(self.parse_expr()?);
        self.expect(TokenKind::Colon)?;
        self.skip_newlines();
        self.expect(TokenKind::Indent)?;
        let mut arms = Vec::new();
        while !self.check(TokenKind::Dedent) && !self.at_end() {
            self.skip_newlines();
            if self.check(TokenKind::Dedent) { break; }
            let arm_start = self.current_span();
            let pattern = self.parse_pattern()?;
            self.expect(TokenKind::FatArrow)?;
            
            // Parse arm body - check if it starts with newline (multi-line body)
            let body = if matches!(self.peek_kind(), TokenKind::Newline(_)) {
                // Multi-line match arm body
                self.skip_newlines();
                
                if self.check(TokenKind::Indent) {
                    // It's an indented block - parse statements until dedent
                    self.advance(); // consume Indent
                    let mut stmts = Vec::new();
                    
                    while !self.check(TokenKind::Dedent) && !self.at_end() {
                        self.skip_newlines();
                        if self.check(TokenKind::Dedent) { break; }
                        stmts.push(self.parse_stmt()?);
                        self.skip_newlines();
                    }
                    
                    if self.check(TokenKind::Dedent) { 
                        self.advance(); // consume Dedent for arm body
                    }
                    
                    // Convert stmts to expression
                    if stmts.len() == 1 {
                        if let Stmt::Expr(e) = &stmts[0] {
                            e.clone()
                        } else if let Stmt::Return(Some(ref e), _) = &stmts[0] {
                            e.clone()
                        } else {
                            let block = Block { stmts, span: arm_start.merge(self.current_span()) };
                            Expr::Block(block, arm_start.merge(self.current_span()))
                        }
                    } else {
                        let block = Block { stmts, span: arm_start.merge(self.current_span()) };
                        Expr::Block(block, arm_start.merge(self.current_span()))
                    }
                } else {
                    // Just an expression on the next line (no indent)
                    self.parse_expr()?
                }
            } else {
                // Inline expression (same line as =>)
                self.parse_expr()?
            };
            
            arms.push(MatchArm { pattern, guard: None, body, span: self.current_span() });
            self.skip_newlines();
        }
        if self.check(TokenKind::Dedent) { self.advance(); }
        Ok(Expr::Match { scrutinee, arms, span: start.merge(self.current_span()) })
    }

    fn parse_if(&mut self) -> KoreResult<Expr> {
        let start = self.current_span();
        self.expect(TokenKind::If)?;
        let condition = Box::new(self.parse_expr()?);
        self.expect(TokenKind::Colon)?;
        
        // Check if this is an inline if (no newline/indent) or block if
        let is_block = matches!(self.peek_kind(), TokenKind::Newline(_) | TokenKind::Indent);
        let then_branch = if is_block {
            self.parse_block()?
        } else {
            // Inline if: parse single statement
            let stmt = self.parse_stmt()?;
            Block { stmts: vec![stmt], span: start.merge(self.current_span()) }
        };
        
        let else_branch = if self.check(TokenKind::Else) {
            self.advance();
            
            // Check for 'else if' (elif pattern) - no colon between else and if
            if self.check(TokenKind::If) {
                // Parse the 'if' expression
                let elif_expr = self.parse_if()?;
                
                // Extract the condition, then_branch, and else_branch from the If expression
                if let Expr::If { condition, then_branch, else_branch: nested_else, .. } = elif_expr {
                    Some(Box::new(ElseBranch::ElseIf(condition, then_branch, nested_else)))
                } else {
                    // Shouldn't happen, but fallback
                    return Err(KoreError::parser("Expected if expression after else", self.current_span()));
                }
            } else {
                self.expect(TokenKind::Colon)?;
                let is_block = matches!(self.peek_kind(), TokenKind::Newline(_) | TokenKind::Indent);
                if is_block {
                    Some(Box::new(ElseBranch::Else(self.parse_block()?)))
                } else {
                    let stmt = self.parse_stmt()?;
                    Some(Box::new(ElseBranch::Else(Block { stmts: vec![stmt], span: start.merge(self.current_span()) })))
                }
            }
        } else { None };
        Ok(Expr::If { condition, then_branch, else_branch, span: start.merge(self.current_span()) })
    }

    fn parse_pattern(&mut self) -> KoreResult<Pattern> {
        let span = self.current_span();
        match self.peek_kind() {
            TokenKind::Ident(ref s) if s == "_" => { self.advance(); Ok(Pattern::Wildcard(span)) }
            TokenKind::Ident(ref s) => { 
                let name = s.clone(); 
                self.advance(); 
                
                if self.check(TokenKind::ColonColon) {
                    self.advance(); // consume ::
                    let variant = self.parse_ident()?;
                    
                    let fields = if self.check(TokenKind::LParen) {
                        self.advance();
                        let mut patterns = Vec::new();
                        while !self.check(TokenKind::RParen) {
                            patterns.push(self.parse_pattern()?);
                            if !self.check(TokenKind::RParen) {
                                self.expect(TokenKind::Comma)?;
                            }
                        }
                        self.expect(TokenKind::RParen)?;
                        VariantPatternFields::Tuple(patterns)
                    } else if self.check(TokenKind::LBrace) {
                        self.advance();
                        let mut fields = Vec::new();
                        while !self.check(TokenKind::RBrace) {
                            let fname = self.parse_ident()?;
                            self.expect(TokenKind::Colon)?;
                            let pat = self.parse_pattern()?;
                            fields.push((fname, pat));
                            if !self.check(TokenKind::RBrace) {
                                self.expect(TokenKind::Comma)?;
                            }
                        }
                        self.expect(TokenKind::RBrace)?;
                        VariantPatternFields::Struct(fields)
                    } else {
                        VariantPatternFields::Unit
                    };
                    
                    Ok(Pattern::Variant {
                        enum_name: Some(name),
                        variant,
                        fields,
                        span: span.merge(self.current_span()),
                    })
                } else if self.check(TokenKind::LParen) {
                    // Unqualified variant pattern: Variant(args) without EnumName::
                    // Common in Python-style pattern matching
                    self.advance(); // consume (
                    let mut patterns = Vec::new();
                    while !self.check(TokenKind::RParen) {
                        patterns.push(self.parse_pattern()?);
                        if !self.check(TokenKind::RParen) {
                            self.expect(TokenKind::Comma)?;
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                    
                    Ok(Pattern::Variant {
                        enum_name: None, // Unqualified - will be resolved at type-check time
                        variant: name,
                        fields: VariantPatternFields::Tuple(patterns),
                        span: span.merge(self.current_span()),
                    })
                } else {
                    Ok(Pattern::Binding { name, mutable: false, span }) 
                }
            }
            TokenKind::Mut => {
                self.advance();
                let name = self.parse_ident()?;
                Ok(Pattern::Binding { name, mutable: true, span: span.merge(self.current_span()) })
            }
            TokenKind::Int(n) => { self.advance(); Ok(Pattern::Literal(Expr::Int(n, span))) }
            TokenKind::String(ref s) => { 
                let string_val = s.clone();
                self.advance(); 
                Ok(Pattern::Literal(Expr::String(string_val, span))) 
            }
            TokenKind::True => { self.advance(); Ok(Pattern::Literal(Expr::Bool(true, span))) }
            TokenKind::False => { self.advance(); Ok(Pattern::Literal(Expr::Bool(false, span))) }
            TokenKind::LParen => {
                self.advance();
                let mut patterns = Vec::new();
                while !self.check(TokenKind::RParen) {
                    patterns.push(self.parse_pattern()?);
                    if !self.check(TokenKind::RParen) { self.expect(TokenKind::Comma)?; }
                }
                self.expect(TokenKind::RParen)?;
                Ok(Pattern::Tuple(patterns, span.merge(self.current_span())))
            }
            TokenKind::LBracket => {
                self.advance();
                let mut patterns = Vec::new();
                while !self.check(TokenKind::RBracket) {
                    patterns.push(self.parse_pattern()?);
                    if !self.check(TokenKind::RBracket) { self.expect(TokenKind::Comma)?; }
                }
                self.expect(TokenKind::RBracket)?;
                Ok(Pattern::Slice { patterns, rest: None, span: span.merge(self.current_span()) })
            }
            _ => Err(KoreError::parser("Expected pattern", span)),
        }
    }

    #[allow(dead_code)]
    fn parse_jsx(&mut self) -> KoreResult<JSXNode> {
        self.skip_newlines();
        self.expect(TokenKind::Indent)?;
        self.skip_newlines();
        let result = self.parse_jsx_element()?;
        self.skip_newlines();
        if self.check(TokenKind::Dedent) { self.advance(); }
        Ok(result)
    }

    fn parse_jsx_element(&mut self) -> KoreResult<JSXNode> {
        let start = self.current_span();
        self.expect(TokenKind::Lt)?;
        let tag = self.parse_ident()?;
        let mut attrs = Vec::new();
        while !self.check(TokenKind::Gt) && !self.check(TokenKind::Slash) {
            let name = self.parse_ident()?;
            self.expect(TokenKind::Eq)?;
            let value = if self.check(TokenKind::LBrace) {
                self.advance();
                let e = self.parse_expr()?;
                self.expect(TokenKind::RBrace)?;
                JSXAttrValue::Expr(e)
            } else if let TokenKind::String(s) = self.peek_kind() {
                self.advance();
                JSXAttrValue::String(s)
            } else {
                return Err(KoreError::parser("Expected attribute value", self.current_span()));
            };
            attrs.push(JSXAttribute { name, value, span: self.current_span() });
        }
        
        if self.check(TokenKind::Slash) {
            self.advance();
            self.expect(TokenKind::Gt)?;
            return Ok(JSXNode::Element { tag, attributes: attrs, children: vec![], span: start.merge(self.current_span()) });
        }
        
        self.expect(TokenKind::Gt)?;
        
        let mut children = Vec::new();
        // Track the end of the previous token to detect gaps (whitespace)
        let mut last_end = self.tokens.get(self.pos - 1).map(|t| t.span.end).unwrap_or(0);
        let mut text_buffer = String::new();
        let mut text_start = self.current_span();

        while !self.check(TokenKind::LtSlash) && !self.at_end() {
            let current_span = self.current_span();
            
            // Check for gap (whitespace)
            if current_span.start > last_end {
                // If we have text in buffer, append space. If buffer empty, maybe leading space?
                // For simplicity, just append space if buffer not empty, or if we want to preserve spacing.
                // But JSX usually collapses whitespace.
                // However, "Count is: {count}" needs space.
                // Let's unconditionally add space if gap detected, but handle collapse later?
                // No, let's just add space.
                if !text_buffer.is_empty() {
                    text_buffer.push(' ');
                }
            }

            if self.check(TokenKind::LBrace) {
                if !text_buffer.is_empty() {
                    children.push(JSXNode::Text(text_buffer.clone(), text_start.merge(Span::new(last_end, last_end))));
                    text_buffer.clear();
                }

                self.advance();
                let expr = self.parse_expr()?;
                self.expect(TokenKind::RBrace)?;
                children.push(JSXNode::Expression(Box::new(expr)));
                
                last_end = self.tokens.get(self.pos - 1).map(|t| t.span.end).unwrap_or(0);
                text_start = self.current_span(); // Reset text start for next text run
            } else if self.check(TokenKind::Lt) {
                 if !text_buffer.is_empty() {
                    children.push(JSXNode::Text(text_buffer.clone(), text_start.merge(Span::new(last_end, last_end))));
                    text_buffer.clear();
                }

                 children.push(self.parse_jsx_element()?);
                 
                 last_end = self.tokens.get(self.pos - 1).map(|t| t.span.end).unwrap_or(0);
                 text_start = self.current_span();
            } else {
                 let mut consumed_text = None;
                 match self.peek_kind() {
                     TokenKind::String(s) => consumed_text = Some(s),
                     TokenKind::Ident(s) => consumed_text = Some(s),
                     TokenKind::Int(n) => consumed_text = Some(n.to_string()),
                     TokenKind::Newline(_) | TokenKind::Indent | TokenKind::Dedent => {
                         // Treat newline/indent as whitespace
                         if !text_buffer.is_empty() && !text_buffer.ends_with(' ') {
                             text_buffer.push(' ');
                         }
                         self.advance();
                     }
                     TokenKind::Colon => consumed_text = Some(":".to_string()),
                     TokenKind::Comma => consumed_text = Some(",".to_string()),
                     TokenKind::Dot => consumed_text = Some(".".to_string()),
                     TokenKind::Question => consumed_text = Some("?".to_string()),
                     TokenKind::Not => consumed_text = Some("!".to_string()),
                     TokenKind::Minus => consumed_text = Some("-".to_string()),
                     TokenKind::Eq => consumed_text = Some("=".to_string()),
                     _ => {
                         return Err(KoreError::parser(format!("Unexpected token in JSX child: {:?}. Use strings or {{}} for text.", self.peek_kind()), self.current_span()));
                     }
                 }
                 
                 if let Some(t) = consumed_text {
                     if text_buffer.is_empty() {
                         text_start = self.current_span();
                     }
                     text_buffer.push_str(&t);
                     self.advance();
                 }
                 
                 last_end = self.tokens.get(self.pos - 1).map(|t| t.span.end).unwrap_or(0);
            }
        }
        
        if !text_buffer.is_empty() {
            children.push(JSXNode::Text(text_buffer, text_start.merge(Span::new(last_end, last_end))));
        }
        
        self.expect(TokenKind::LtSlash)?;
        let closing_tag = self.parse_ident()?;
        if closing_tag != tag {
            return Err(KoreError::parser(format!("Expected closing tag </{}>, found </{}>", tag, closing_tag), self.current_span()));
        }
        self.expect(TokenKind::Gt)?;
        
        Ok(JSXNode::Element { tag, attributes: attrs, children, span: start.merge(self.current_span()) })
    }

    fn parse_call_args(&mut self) -> KoreResult<Vec<CallArg>> {
        let mut args = Vec::new();
        self.skip_formatting();
        while !self.check(TokenKind::RParen) && !self.at_end() {
            let mut name = None;
            // Check for named argument: ident = expr
            if let TokenKind::Ident(s) = self.peek_kind() {
                // Look ahead for '='
                if self.tokens.get(self.pos + 1).map(|t| t.kind == TokenKind::Eq).unwrap_or(false) {
                    name = Some(s);
                    self.advance(); // eat ident
                    self.advance(); // eat =
                }
            }
            
            let value = self.parse_expr()?;
            args.push(CallArg { name, value, span: self.current_span() });
            
            self.skip_formatting();
            if !self.check(TokenKind::RParen) { 
                self.expect(TokenKind::Comma)?; 
                self.skip_formatting();
            }
        }
        Ok(args)
    }

    fn parse_visibility(&mut self) -> Visibility {
        if self.check(TokenKind::Pub) { self.advance(); Visibility::Public } else { Visibility::Private }
    }

    fn parse_ident(&mut self) -> KoreResult<String> {
        match self.peek_kind() {
            TokenKind::Ident(s) => { self.advance(); Ok(s) }
            TokenKind::SelfLower => { self.advance(); Ok("self".to_string()) }
            TokenKind::SelfUpper => { self.advance(); Ok("Self".to_string()) }
            k => Err(KoreError::parser(format!("Expected identifier, got {:?}", k), self.current_span())),
        }
    }

    fn get_binary_op(&self) -> Option<(BinaryOp, u8)> {
        match self.peek_kind() {
            TokenKind::Or => Some((BinaryOp::Or, 1)),
            TokenKind::And => Some((BinaryOp::And, 2)),
            TokenKind::EqEq => Some((BinaryOp::Eq, 3)),
            TokenKind::NotEq => Some((BinaryOp::Ne, 3)),
            TokenKind::Lt => Some((BinaryOp::Lt, 4)),
            TokenKind::Gt => Some((BinaryOp::Gt, 4)),
            TokenKind::LtEq => Some((BinaryOp::Le, 4)),
            TokenKind::GtEq => Some((BinaryOp::Ge, 4)),
            TokenKind::Plus => Some((BinaryOp::Add, 5)),
            TokenKind::Minus => Some((BinaryOp::Sub, 5)),
            TokenKind::Star => Some((BinaryOp::Mul, 6)),
            TokenKind::Slash => Some((BinaryOp::Div, 6)),
            TokenKind::Percent => Some((BinaryOp::Mod, 6)),
            TokenKind::Power => Some((BinaryOp::Pow, 7)),
            _ => None,
        }
    }

    // Helper methods
    fn peek_kind(&self) -> TokenKind { self.tokens.get(self.pos).map(|t| t.kind.clone()).unwrap_or(TokenKind::Eof) }
    fn current_span(&self) -> Span { self.tokens.get(self.pos).map(|t| t.span).unwrap_or(Span::new(0, 0)) }
    fn at_end(&self) -> bool { matches!(self.peek_kind(), TokenKind::Eof) }
    fn check(&self, k: TokenKind) -> bool { std::mem::discriminant(&self.peek_kind()) == std::mem::discriminant(&k) }
    fn check_line_end(&self) -> bool { matches!(self.peek_kind(), TokenKind::Newline(_) | TokenKind::Dedent | TokenKind::Eof) }
    fn advance(&mut self) { if !self.at_end() { self.pos += 1; } }
    fn skip_newlines(&mut self) { while let TokenKind::Newline(_) = self.peek_kind() { self.advance(); } }
    fn check_newline(&self) -> bool { matches!(self.peek_kind(), TokenKind::Newline(_)) }
    fn skip_formatting(&mut self) {
        while matches!(self.peek_kind(), TokenKind::Newline(_) | TokenKind::Indent | TokenKind::Dedent) {
            self.advance();
        }
    }

    fn expect(&mut self, k: TokenKind) -> KoreResult<()> {
        if self.check(k.clone()) { self.advance(); Ok(()) }
        else { Err(KoreError::parser(format!("Expected {:?}, got {:?}", k, self.peek_kind()), self.current_span())) }
    }
}

