//! Error types for the KAIN compiler

use crate::span::Span;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum KainError {
    #[error("Lexer error at {span:?}: {message}")]
    Lexer { message: String, span: Span },

    #[error("Parser error at {span:?}: {message}")]
    Parser { message: String, span: Span },

    #[error("Type error at {span:?}: {message}")]
    Type { message: String, span: Span },

    #[error("Effect error at {span:?}: {message}")]
    Effect { message: String, span: Span },

    #[error("Borrow error at {span:?}: {message}")]
    Borrow { message: String, span: Span },

    #[error("Codegen error at {span:?}: {message}")]
    Codegen { message: String, span: Span },

    #[error("Runtime error: {message}")]
    Runtime { message: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl KainError {
    pub fn lexer(message: impl Into<String>, span: Span) -> Self {
        KainError::Lexer {
            message: message.into(),
            span,
        }
    }

    pub fn parser(message: impl Into<String>, span: Span) -> Self {
        KainError::Parser {
            message: message.into(),
            span,
        }
    }

    pub fn type_error(message: impl Into<String>, span: Span) -> Self {
        KainError::Type {
            message: message.into(),
            span,
        }
    }

    pub fn effect_error(message: impl Into<String>, span: Span) -> Self {
        KainError::Effect {
            message: message.into(),
            span,
        }
    }

    pub fn borrow_error(message: impl Into<String>, span: Span) -> Self {
        KainError::Borrow {
            message: message.into(),
            span,
        }
    }

    pub fn codegen(message: impl Into<String>, span: Span) -> Self {
        KainError::Codegen {
            message: message.into(),
            span,
        }
    }

    pub fn runtime(message: impl Into<String>) -> Self {
        KainError::Runtime {
            message: message.into(),
        }
    }
}

/// Result type for KAIN operations
pub type KainResult<T> = Result<T, KainError>;

