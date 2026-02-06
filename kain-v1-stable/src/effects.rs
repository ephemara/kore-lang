//! KAIN Effect System - Track side effects at compile time

use crate::span::Span;
use crate::error::{KainError, KainResult};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Effect {
    Pure,      // No side effects
    IO,        // File/Network/Console
    Async,     // Can await
    GPU,       // Runs on graphics hardware
    Reactive,  // Triggers UI updates
    Unsafe,    // Breaks safety guarantees
    Alloc,     // Memory allocation
    Panic,     // Can abort
}

impl Effect {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "Pure" => Some(Effect::Pure),
            "IO" => Some(Effect::IO),
            "Async" => Some(Effect::Async),
            "GPU" => Some(Effect::GPU),
            "Reactive" => Some(Effect::Reactive),
            "Unsafe" => Some(Effect::Unsafe),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectSet {
    pub effects: HashSet<Effect>,
}

impl EffectSet {
    pub fn new() -> Self { Self { effects: HashSet::new() } }
    pub fn pure() -> Self { Self::new().with(Effect::Pure) }
    
    pub fn with(mut self, e: Effect) -> Self {
        self.effects.insert(e);
        self
    }
    
    pub fn is_pure(&self) -> bool {
        self.effects.is_empty() || self.effects.iter().all(|e| *e == Effect::Pure)
    }
    
    pub fn can_call(&self, callee: &EffectSet) -> bool {
        if callee.is_pure() { return true; }
        if self.is_pure() { return false; }
        if self.effects.contains(&Effect::Unsafe) { return true; }
        callee.effects.iter().all(|e| self.effects.contains(e))
    }
}

pub fn check_effect_call(caller: &EffectSet, callee: &EffectSet, span: Span) -> KainResult<()> {
    if !caller.can_call(callee) {
        return Err(KainError::effect_error(
            format!("Effect violation: {:?} cannot call {:?}", caller.effects, callee.effects),
            span,
        ));
    }
    Ok(())
}

