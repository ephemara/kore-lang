//! Pretty error reporting for KAIN
//! Shows source context with line numbers and error highlighting

use crate::span::Span;
use crate::error::KainError;

/// Diagnostic renderer for pretty error messages
pub struct Diagnostics<'a> {
    source: &'a str,
    filename: &'a str,
}

impl<'a> Diagnostics<'a> {
    pub fn new(source: &'a str, filename: &'a str) -> Self {
        Self { source, filename }
    }
    
    /// Format an error with source context
    pub fn format_error(&self, error: &KainError) -> String {
        match error {
            KainError::Lexer { message, span } => self.format_with_context("Lexer Error", message, *span),
            KainError::Parser { message, span } => self.format_with_context("Parse Error", message, *span),
            KainError::Type { message, span } => self.format_with_context("Type Error", message, *span),
            KainError::Effect { message, span } => self.format_with_context("Effect Error", message, *span),
            KainError::Borrow { message, span } => self.format_with_context("Borrow Error", message, *span),
            KainError::Codegen { message, span } => self.format_with_context("Codegen Error", message, *span),
            KainError::Runtime { message } => format!(
                "\n\x1b[1;31merror\x1b[0m: {}\n",
                message
            ),
            KainError::Io(e) => format!(
                "\n\x1b[1;31merror\x1b[0m: IO error: {}\n",
                e
            ),
        }
    }
    
    fn format_with_context(&self, error_type: &str, message: &str, span: Span) -> String {
        let (line_num, col, line_content) = self.get_line_info(span);
        
        let mut output = String::new();
        
        // Error header
        output.push_str(&format!(
            "\n\x1b[1;31merror[{}]\x1b[0m: {}\n",
            error_type, message
        ));
        
        // Location
        output.push_str(&format!(
            "  \x1b[1;34m-->\x1b[0m {}:{}:{}\n",
            self.filename, line_num, col
        ));
        
        // Separator
        output.push_str("   \x1b[1;34m|\x1b[0m\n");
        
        // Source line
        output.push_str(&format!(
            "\x1b[1;34m{:>3} |\x1b[0m {}\n",
            line_num, line_content
        ));
        
        // Error pointer
        let pointer_offset = col.saturating_sub(1);
        let content_len = line_content.len();
        let remaining_len = content_len.saturating_sub(pointer_offset);
        let span_len = span.end.saturating_sub(span.start);
        let pointer_len = span_len.min(remaining_len).max(1);
        
        output.push_str(&format!(
            "   \x1b[1;34m|\x1b[0m {}\x1b[1;31m{}\x1b[0m\n",
            " ".repeat(pointer_offset),
            "^".repeat(pointer_len)
        ));
        
        // Separator
        output.push_str("   \x1b[1;34m|\x1b[0m\n");
        
        output
    }
    
    /// Get line number, column, and line content for a span
    fn get_line_info(&self, span: Span) -> (usize, usize, &str) {
        let mut line_num = 1;
        let mut line_start = 0;
        
        // Safety check for span bounds
        let start = span.start.min(self.source.len());
        
        for (i, c) in self.source.char_indices() {
            if i >= start {
                break;
            }
            if c == '\n' {
                line_num += 1;
                line_start = i + 1;
            }
        }
        
        let col = start.saturating_sub(line_start) + 1;
        
        // Find line end
        let line_end = if start < self.source.len() {
            self.source[start..]
                .find('\n')
                .map(|i| start + i)
                .unwrap_or(self.source.len())
        } else {
            self.source.len()
        };
        
        let line_start = line_start.min(self.source.len());
        let line_content = &self.source[line_start..line_end];
        
        (line_num, col, line_content)
    }
}

/// Format an error without source context (for runtime errors)
pub fn format_simple_error(error: &KainError) -> String {
    match error {
        KainError::Runtime { message } => format!("Runtime Error: {}", message),
        _ => format!("{}", error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_line_info() {
        let source = "let x = 5\nlet y = x + 1\nprint(y)";
        let diag = Diagnostics::new(source, "test.kn");
        
        // First line
        let (line, col, content) = diag.get_line_info(Span::new(0, 3));
        assert_eq!(line, 1);
        assert_eq!(col, 1);
        assert_eq!(content, "let x = 5");
        
        // Second line
        let (line, col, content) = diag.get_line_info(Span::new(14, 15));
        assert_eq!(line, 2);
        assert_eq!(content, "let y = x + 1");
    }
}

