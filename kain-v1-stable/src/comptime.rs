use crate::ast::*;
use crate::runtime::{Env, eval_expr, Value};
use crate::error::KainResult;
use crate::span::Span;

pub fn eval_program(program: &mut Program) -> KainResult<()> {
    let mut env = Env::new();
    
    for item in &mut program.items {
        eval_item(&mut env, item)?;
    }
    
    Ok(())
}

fn eval_item(env: &mut Env, item: &mut Item) -> KainResult<()> {
    match item {
        Item::Function(f) => eval_block(env, &mut f.body)?,
        Item::Comptime(block) => {
            crate::runtime::eval_block(env, &block.body)?;
        }
        Item::Component(c) => {
             eval_jsx(env, &mut c.body)?;
             for method in &mut c.methods {
                 eval_block(env, &mut method.body)?;
             }
        }
        Item::Const(c) => {
            eval_expr_in_place(env, &mut c.value)?;
        }
        _ => {}
    }
    Ok(())
}

fn eval_block(env: &mut Env, block: &mut Block) -> KainResult<()> {
    for stmt in &mut block.stmts {
        eval_stmt(env, stmt)?;
    }
    Ok(())
}

fn eval_stmt(env: &mut Env, stmt: &mut Stmt) -> KainResult<()> {
    match stmt {
        Stmt::Let { value: Some(e), .. } => eval_expr_in_place(env, e)?,
        Stmt::Expr(e) => eval_expr_in_place(env, e)?,
        Stmt::Return(Some(e), _) => eval_expr_in_place(env, e)?,
        Stmt::For { iter, body, .. } => {
            eval_expr_in_place(env, iter)?;
            eval_block(env, body)?;
        }
        _ => {}
    }
    Ok(())
}

fn eval_expr_in_place(env: &mut Env, expr: &mut Expr) -> KainResult<()> {
    // Check if this IS a comptime expression
    if let Expr::Comptime(inner, span) = expr {
        // Evaluate inner expression
        let val = eval_expr(env, inner)?;
        
        // Replace current expr with result value (Literal)
        *expr = value_to_expr(val, *span);
        return Ok(());
    }
    
    // Otherwise recurse
    match expr {
        Expr::Binary { left, right, .. } => {
            eval_expr_in_place(env, left)?;
            eval_expr_in_place(env, right)?;
        }
        Expr::Call { args, .. } => {
             for arg in args {
                 eval_expr_in_place(env, &mut arg.value)?;
             }
        }
        Expr::Assign { value, .. } => eval_expr_in_place(env, value)?,
        Expr::Paren(e, _) => eval_expr_in_place(env, e)?,
        Expr::Block(b, _) => eval_block(env, b)?,
        Expr::JSX(node, _) => eval_jsx(env, node)?,
        _ => {}
    }
    Ok(())
}

fn eval_jsx(env: &mut Env, node: &mut JSXNode) -> KainResult<()> {
    match node {
        JSXNode::Element { attributes, children, .. } => {
             for attr in attributes {
                 if let JSXAttrValue::Expr(e) = &mut attr.value {
                     eval_expr_in_place(env, e)?;
                 }
             }
             for child in children {
                 eval_jsx(env, child)?;
             }
        }
        JSXNode::Expression(e) => eval_expr_in_place(env, e)?,
        _ => {}
    }
    Ok(())
}

fn value_to_expr(val: Value, span: Span) -> Expr {
    match val {
        Value::Int(n) => Expr::Int(n, span),
        Value::Float(n) => Expr::Float(n, span),
        Value::Bool(b) => Expr::Bool(b, span),
        Value::String(s) => Expr::String(s, span),
        Value::Unit => Expr::Block(Block { stmts: vec![], span }, span), // Unit is empty block?
        _ => Expr::String(format!("<unrepresentable comptime value: {}>", val), span),
    }
}

