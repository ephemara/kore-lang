use std::collections::{HashMap, hash_map::Entry};
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use crate::ast::{Program, Item, Function, Type};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::span::Span;
use crate::error::KainError;
use crate::types;

#[derive(Debug, Clone)]
struct Document {
    text: String,
    version: i32,
    analysis: Option<DocumentAnalysis>,
}

#[derive(Debug, Default)]
struct DocumentStore {
    docs: tokio::sync::RwLock<HashMap<Url, Document>>,
}

impl DocumentStore {
    async fn upsert(&self, uri: Url, text: String, version: i32) {
        let mut guard = self.docs.write().await;
        guard.insert(uri, Document { text, version, analysis: None });
    }

    async fn remove(&self, uri: &Url) {
        let mut guard = self.docs.write().await;
        guard.remove(uri);
    }

    async fn get_text(&self, uri: &Url) -> Option<String> {
        let guard = self.docs.read().await;
        guard.get(uri).map(|doc| doc.text.clone())
    }

    async fn get_analysis(&self, uri: &Url) -> Option<DocumentAnalysis> {
        let guard = self.docs.read().await;
        guard.get(uri).and_then(|doc| doc.analysis.clone())
    }

    async fn update_analysis(&self, uri: &Url, analysis: Option<DocumentAnalysis>) {
        let mut guard = self.docs.write().await;
        if let Some(doc) = guard.get_mut(uri) {
            doc.analysis = analysis;
        }
    }

    async fn apply_changes(
        &self,
        uri: &Url,
        version: i32,
        changes: &[TextDocumentContentChangeEvent],
    ) -> Option<String> {
        if changes.is_empty() {
            return self.get_text(uri).await;
        }

        let mut guard = self.docs.write().await;
        match guard.entry(uri.clone()) {
            Entry::Occupied(mut entry) => {
                let mut current = entry.get().text.clone();
                for change in changes {
                    current = if let Some(range) = &change.range {
                        match apply_change(&current, range, &change.text) {
                            Some(updated) => updated,
                            None => return None,
                        }
                    } else {
                        change.text.clone()
                    };
                }
                {
                    let doc = entry.get_mut();
                    doc.text = current.clone();
                    doc.version = version;
                    doc.analysis = None;
                }
                Some(current)
            }
            Entry::Vacant(entry) => {
                // Only allow creating a new document via a full sync payload
                let full_text_change = changes.iter().rev().find(|c| c.range.is_none())?;
                let text = full_text_change.text.clone();
                entry.insert(Document { text: text.clone(), version, analysis: None });
                Some(text)
            }
        }
    }
}

#[derive(Debug, Clone)]
struct DocumentAnalysis {
    symbols: HashMap<String, Vec<SymbolInfo>>,
}

#[derive(Debug, Clone)]
struct SymbolInfo {
    range: Range,
    detail: Option<String>,
    kind: SymbolKind,
}

impl DocumentAnalysis {
    fn from_program(text: &str, program: &Program) -> Self {
        let mut symbols: HashMap<String, Vec<SymbolInfo>> = HashMap::new();

        for item in &program.items {
            if let Item::Function(func) = item {
                if let Some(range) = find_identifier_range(text, &func.name, Some(func.span)) {
                    let detail = Some(format_fn_signature(func));
                    symbols.entry(func.name.clone())
                        .or_default()
                        .push(SymbolInfo { range, detail, kind: SymbolKind::Function });
                }

                for param in &func.params {
                    if let Some(range) = find_identifier_range(text, &param.name, Some(param.span)) {
                        let detail = Some(format!("param {}: {}", param.name, format_type(&param.ty)));
                        symbols.entry(param.name.clone())
                            .or_default()
                            .push(SymbolInfo { range, detail, kind: SymbolKind::Variable });
                    }
                }
            }
        }

        Self { symbols }
    }

    fn lookup(&self, ident: &str) -> Option<&[SymbolInfo]> {
        self.symbols.get(ident).map(|v| v.as_slice())
    }
}

#[derive(Debug, Clone, Copy)]
enum SymbolKind {
    Function,
    Variable,
}

impl SymbolKind {
    fn completion_item_kind(self) -> CompletionItemKind {
        match self {
            SymbolKind::Function => CompletionItemKind::FUNCTION,
            SymbolKind::Variable => CompletionItemKind::VARIABLE,
        }
    }
}

fn format_fn_signature(function: &Function) -> String {
    let params = function.params
        .iter()
        .map(|p| format!("{}: {}", p.name, format_type(&p.ty)))
        .collect::<Vec<_>>()
        .join(", ");

    let ret = function.return_type
        .as_ref()
        .map(|t| format_type(t))
        .unwrap_or_else(|| "()".to_string());

    format!("fn {}({}) -> {}", function.name, params, ret)
}

fn format_type(ty: &Type) -> String {
    match ty {
        Type::Named { name, generics, .. } => {
            if generics.is_empty() {
                name.clone()
            } else {
                format!(
                    "{}<{}>",
                    name,
                    generics.iter().map(format_type).collect::<Vec<_>>().join(", ")
                )
            }
        }
        Type::Tuple(items, _) => format!(
            "({})",
            items.iter().map(format_type).collect::<Vec<_>>().join(", ")
        ),
        Type::Array(inner, size, _) => format!("[{}; {}]", format_type(inner), size),
        Type::Slice(inner, _) => format!("[{}]", format_type(inner)),
        Type::Ref { mutable, inner, .. } => {
            if *mutable {
                format!("&mut {}", format_type(inner))
            } else {
                format!("&{}", format_type(inner))
            }
        }
        Type::Function { params, return_type, .. } => format!(
            "fn({}) -> {}",
            params.iter().map(format_type).collect::<Vec<_>>().join(", "),
            format_type(return_type),
        ),
        Type::Option(inner, _) => format!("{}?", format_type(inner)),
        Type::Result(ok, err, _) => format!("{}!{}", format_type(ok), format_type(err)),
        Type::Infer(_) => "_".into(),
        Type::Never(_) => "!".into(),
        Type::Unit(_) => "()".into(),
        Type::Impl { trait_name, generics, .. } => {
            if generics.is_empty() {
                format!("impl {}", trait_name)
            } else {
                format!(
                    "impl {}<{}>",
                    trait_name,
                    generics.iter().map(format_type).collect::<Vec<_>>().join(", ")
                )
            }
        }
    }
}

fn find_identifier_range(text: &str, name: &str, span_hint: Option<Span>) -> Option<Range> {
    let bytes = text.as_bytes();
    if bytes.is_empty() || name.is_empty() {
        return None;
    }

    let (start, end) = if let Some(span) = span_hint {
        let s = span.start.min(text.len());
        let e = span.end.min(text.len());
        (s, e)
    } else {
        (0, text.len())
    };

    let window = &text[start..end];
    if let Some(idx) = window.find(name) {
        let absolute = start + idx;
        let span = Span::new(absolute, absolute + name.len());
        return Some(span_to_range(text, span));
    }

    if let Some(idx) = text.find(name) {
        let span = Span::new(idx, idx + name.len());
        return Some(span_to_range(text, span));
    }

    None
}

fn find_ident_at_offset(text: &str, offset: usize) -> Option<(String, Range)> {
    if offset > text.len() {
        return None;
    }

    let bytes = text.as_bytes();
    let mut start = offset;
    let mut end = offset;

    while start > 0 && is_ident_char(bytes[start - 1] as char) {
        start -= 1;
    }
    while end < bytes.len() && is_ident_char(bytes[end] as char) {
        end += 1;
    }

    if start == end {
        return None;
    }

    let ident = String::from_utf8_lossy(&bytes[start..end]).to_string();
    let start_pos = offset_to_position_standalone(text, start);
    let end_pos = offset_to_position_standalone(text, end);
    Some((ident, Range { start: start_pos, end: end_pos }))
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn diagnostic_from_error(text: &str, err: &KainError) -> Vec<Diagnostic> {
    let (message, span) = match err {
        KainError::Lexer { message, span } => (message.clone(), *span),
        KainError::Parser { message, span } => (message.clone(), *span),
        KainError::Type { message, span } => (message.clone(), *span),
        KainError::Effect { message, span } => (message.clone(), *span),
        KainError::Borrow { message, span } => (message.clone(), *span),
        KainError::Codegen { message, span } => (message.clone(), *span),
        KainError::Runtime { message } => (message.clone(), Span::default()),
        KainError::Io(_) => return vec![],
    };

    let range = span_to_range(text, span);
    vec![Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some("KAIN".to_string()),
        message,
        related_information: None,
        tags: None,
        data: None,
    }]
}

fn span_to_range(text: &str, span: Span) -> Range {
    let start = offset_to_position_standalone(text, span.start);
    let end = offset_to_position_standalone(text, span.end);
    Range { start, end }
}

#[derive(Debug)]
struct Backend {
    client: Client,
    docs: DocumentStore,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::INCREMENTAL),
                        will_save: None,
                        will_save_wait_until: None,
                        save: None,
                    }
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions::default()),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "KAIN Language Server initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        let version = params.text_document.version;
        self.docs.upsert(uri.clone(), text.clone(), version).await;
        self.validate_document(uri, text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if params.content_changes.is_empty() {
            return;
        }

        let uri = params.text_document.uri;
        let version = params.text_document.version;
        match self.docs.apply_changes(&uri, version, &params.content_changes).await {
            Some(text) => {
                self.validate_document(uri.clone(), text).await;
            }
            None => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!("Failed to apply text changes for {}", uri),
                    )
                    .await;
            }
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.docs.remove(&params.text_document.uri).await;
        // Clear diagnostics on close
        self.client.publish_diagnostics(params.text_document.uri, vec![], None).await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let text = match self.docs.get_text(&uri).await {
            Some(t) => t,
            None => return Ok(None),
        };

        let analysis = match self.docs.get_analysis(&uri).await {
            Some(a) => a,
            None => return Ok(None),
        };

        let offset = match position_to_offset(&text, &position) {
            Some(o) => o,
            None => return Ok(None),
        };

        let (ident, _range) = match find_ident_at_offset(&text, offset) {
            Some(res) => res,
            None => return Ok(None),
        };

        if let Some(symbols) = analysis.lookup(&ident) {
            if let Some(info) = symbols.first() {
                let contents = HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: info.detail.clone().unwrap_or_else(|| ident.clone()),
                });
                return Ok(Some(Hover {
                    contents,
                    range: Some(info.range),
                }));
            }
        }

        Ok(None)
    }

    async fn goto_definition(&self, params: GotoDefinitionParams) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let text = match self.docs.get_text(&uri).await {
            Some(t) => t,
            None => return Ok(None),
        };

        let analysis = match self.docs.get_analysis(&uri).await {
            Some(a) => a,
            None => return Ok(None),
        };

        let offset = match position_to_offset(&text, &position) {
            Some(o) => o,
            None => return Ok(None),
        };

        let (ident, _) = match find_ident_at_offset(&text, offset) {
            Some(res) => res,
            None => return Ok(None),
        };

        if let Some(infos) = analysis.lookup(&ident) {
            if let Some(info) = infos.first() {
                let loc = Location::new(uri.clone(), info.range.clone());
                return Ok(Some(GotoDefinitionResponse::Scalar(loc)));
            }
        }

        Ok(None)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let text = self.docs.get_text(&uri).await.unwrap_or_default();
        let analysis = match self.docs.get_analysis(&uri).await {
            Some(a) => a,
            None => return Ok(None),
        };

        let mut items = Vec::new();
        for (name, infos) in analysis.symbols.iter() {
            if let Some(info) = infos.first() {
                items.push(CompletionItem {
                    label: name.clone(),
                    kind: Some(info.kind.completion_item_kind()),
                    detail: info.detail.clone(),
                    ..CompletionItem::default()
                });
            }
        }

        // Basic filtering by current ident prefix (optional)
        let pos = params.text_document_position.position;
        if let Some(offset) = position_to_offset(&text, &pos) {
            if let Some((prefix, _)) = find_ident_at_offset(&text, offset) {
                items.retain(|item| item.label.starts_with(&prefix));
            }
        }

        Ok(Some(CompletionResponse::Array(items)))
    }
}

impl Backend {
    async fn validate_document(&self, uri: Url, text: String) {
        // Run lexer and parser to get errors
        let lexer = Lexer::new(&text);
        let tokens = match lexer.tokenize() {
            Ok(t) => t,
            Err(e) => {
                let diag = diagnostic_from_error(&text, &e);
                self.client.publish_diagnostics(uri.clone(), diag, None).await;
                self.docs.update_analysis(&uri, None).await;
                return;
            }
        };

        let mut parser = Parser::new(&tokens);
        
        let program = match parser.parse() {
            Ok(p) => p,
            Err(e) => {
                let diag = diagnostic_from_error(&text, &e);
                self.client.publish_diagnostics(uri.clone(), diag, None).await;
                self.docs.update_analysis(&uri, None).await;
                return;
            }
        };

        // Type check for richer diagnostics
        if let Err(e) = types::check(&program) {
            let diag = diagnostic_from_error(&text, &e);
            self.client.publish_diagnostics(uri.clone(), diag, None).await;
            self.docs.update_analysis(&uri, None).await;
            return;
        }

        // Build analysis for hover/definition/completion
        let analysis = DocumentAnalysis::from_program(&text, &program);
        self.docs.update_analysis(&uri, Some(analysis)).await;

        self.client.publish_diagnostics(uri, vec![], None).await;
    }

    fn span_to_range(&self, text: &str, span: crate::span::Span) -> Range {
        // Convert byte offset span to line/col range
        let start = self.offset_to_position(text, span.start);
        let end = self.offset_to_position(text, span.end);
        Range { start, end }
    }

    #[allow(dead_code)]
    fn offset_to_position(&self, text: &str, offset: usize) -> Position {
        let mut line = 0;
        let mut col = 0;
        let mut cur = 0;

        for c in text.chars() {
            if cur >= offset {
                break;
            }
            if c == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
            cur += c.len_utf8();
        }

        Position { line, character: col }
    }
}

pub async fn run_server() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        docs: DocumentStore::default(),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}

fn apply_change(text: &str, range: &Range, new_text: &str) -> Option<String> {
    let start = position_to_offset(text, &range.start)?;
    let end = position_to_offset(text, &range.end)?;
    if start > end || end > text.len() {
        return None;
    }

    let mut result = String::with_capacity(text.len() + new_text.len());
    result.push_str(&text[..start]);
    result.push_str(new_text);
    result.push_str(&text[end..]);
    Some(result)
}

fn position_to_offset(text: &str, position: &Position) -> Option<usize> {
    let mut offset = 0usize;
    let mut line = 0usize;
    let mut col = 0usize;
    let target_line = position.line as usize;
    let target_col = position.character as usize;

    for ch in text.chars() {
        if line == target_line && col == target_col {
            return Some(offset);
        }

        let ch_len = ch.len_utf8();
        offset += ch_len;

        if ch == '\n' {
            if line == target_line && target_col == col {
                return Some(offset);
            }
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }

    if line == target_line && col == target_col {
        Some(offset)
    } else if line == target_line && target_col >= col {
        // Allow positions that extend past the current line (append edits)
        Some(offset)
    } else {
        None
    }
}


fn offset_to_position_standalone(text: &str, offset: usize) -> Position {
    let mut line = 0;
    let mut col = 0;
    let mut cur = 0;

    for c in text.chars() {
        if cur >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
        cur += c.len_utf8();
    }

    Position { line, character: col }
}



