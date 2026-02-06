//! KAIN Runtime - Interpreter and actor system

use crate::ast::*;
use crate::error::{KainError, KainResult};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::types::TypedProgram;
use flume::Sender;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, RwLock};

fn py_to_value(obj: &PyAny) -> PyResult<Value> {
    if let Ok(s) = obj.extract::<String>() {
        return Ok(Value::String(s));
    }
    if let Ok(b) = obj.extract::<bool>() {
        return Ok(Value::Bool(b));
    }
    if let Ok(i) = obj.extract::<i64>() {
        return Ok(Value::Int(i));
    }
    if let Ok(f) = obj.extract::<f64>() {
        return Ok(Value::Float(f));
    }
    if let Ok(l) = obj.downcast::<PyList>() {
        let mut vec = Vec::new();
        for item in l {
            vec.push(py_to_value(item)?);
        }
        return Ok(Value::Array(Arc::new(RwLock::new(vec))));
    }
    // Fallback string representation
    Ok(Value::String(format!("{}", obj)))
}

/// Runtime VDOM Node
#[derive(Clone, Debug)]
pub enum VNode {
    Element {
        tag: String,
        attrs: HashMap<String, Value>,
        children: Vec<VNode>,
    },
    Text(String),
}

/// Runtime value
#[derive(Clone)]
pub enum Value {
    Unit,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Arc<RwLock<Vec<Value>>>),
    Tuple(Vec<Value>),
    Struct(String, Arc<RwLock<HashMap<String, Value>>>),
    Function(String),
    NativeFn(String, fn(&mut Env, Vec<Value>) -> KainResult<Value>),
    ActorRef(ActorRef),
    None,
    /// Special value for return flow control
    Return(Box<Value>),
    /// Break from loop with optional value
    Break(Option<Box<Value>>),
    /// Continue to next loop iteration
    Continue,
    /// Result: Ok(true, val) or Err(false, val)
    Result(bool, Box<Value>),
    /// Closure: params, body, captured_scopes
    Closure(Vec<String>, Box<Expr>, Vec<HashMap<String, Value>>),
    /// Struct Constructor: name, field_names
    StructConstructor(String, Vec<String>),
    /// JSX Element
    JSX(VNode),
    /// Enum variant: (enum_name, variant_name, fields)
    EnumVariant(String, String, Vec<Value>),
    /// Poll result for async: Ready(value) or Pending
    Poll(bool, Option<Box<Value>>),
    /// Future state machine: (struct_name, state_struct, poll_fn_name)
    Future(String, Arc<RwLock<HashMap<String, Value>>>),
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Unit => write!(f, "Unit"),
            Value::Bool(b) => write!(f, "Bool({})", b),
            Value::Int(i) => write!(f, "Int({})", i),
            Value::Float(fl) => write!(f, "Float({})", fl),
            Value::String(s) => write!(f, "String({:?})", s),
            Value::Array(arr) => write!(f, "Array({:?})", arr),
            Value::Tuple(t) => write!(f, "Tuple({:?})", t),
            Value::Struct(name, fields) => write!(f, "Struct({}, {:?})", name, fields),
            Value::Function(name) => write!(f, "Function({})", name),
            Value::NativeFn(name, _) => write!(f, "NativeFn({})", name),
            Value::StructConstructor(name, _) => write!(f, "StructConstructor({})", name),
            Value::ActorRef(r) => write!(f, "ActorRef({:?})", r),
            Value::None => write!(f, "None"),
            Value::Return(v) => write!(f, "Return({:?})", v),
            Value::Result(ok, v) => {
                if *ok {
                    write!(f, "Ok({:?})", v)
                } else {
                    write!(f, "Err({:?})", v)
                }
            }
            Value::Closure(params, _, _) => write!(f, "Closure({:?})", params),
            Value::JSX(node) => write!(f, "JSX({:?})", node),
            Value::EnumVariant(e, v, _) => write!(f, "{}::{}", e, v),
            Value::Poll(ready, val) => {
                if *ready {
                    write!(f, "Poll::Ready({:?})", val)
                } else {
                    write!(f, "Poll::Pending")
                }
            }
            Value::Future(name, _) => write!(f, "Future<{}>", name),
            Value::Break(v) => write!(f, "Break({:?})", v),
            Value::Continue => write!(f, "Continue"),
        }
    }
}

impl fmt::Display for VNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VNode::Element {
                tag,
                attrs,
                children,
            } => {
                write!(f, "<{}", tag)?;
                for (k, v) in attrs {
                    write!(f, " {}=\"{}\"", k, v)?;
                }
                if children.is_empty() {
                    write!(f, " />")
                } else {
                    write!(f, ">")?;
                    for child in children {
                        write!(f, "{}", child)?;
                    }
                    write!(f, "</{}>", tag)
                }
            }
            VNode::Text(s) => write!(f, "{}", s),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Unit => write!(f, "()"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Int(i) => write!(f, "{}", i),
            Value::Float(fl) => write!(f, "{}", fl),
            Value::String(s) => write!(f, "{}", s),
            Value::Array(arr) => {
                write!(f, "[")?;
                let arr = arr.read().unwrap();
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Tuple(t) => {
                write!(f, "(")?;
                for (i, v) in t.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, ")")
            }
            Value::Struct(name, fields) => {
                write!(f, "{} {{", name)?;
                let fields = fields.read().unwrap();
                for (i, (k, v)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k, v)?;
                }
                write!(f, "}}")
            }
            Value::Function(name) => write!(f, "<fn {}>", name),
            Value::NativeFn(name, _) => write!(f, "<native fn {}>", name),
            Value::StructConstructor(name, _) => write!(f, "<constructor {}>", name),
            Value::ActorRef(r) => write!(f, "<actor {}>", r.id),
            Value::None => write!(f, "none"),
            Value::Return(v) => write!(f, "{}", v),
            Value::Result(ok, v) => {
                if *ok {
                    write!(f, "Ok({})", v)
                } else {
                    write!(f, "Err({})", v)
                }
            }
            Value::Closure(_, _, _) => write!(f, "<closure>"),
            Value::JSX(node) => write!(f, "{}", node),
            Value::EnumVariant(enum_name, variant, fields) => {
                if fields.is_empty() {
                    write!(f, "{}::{}", enum_name, variant)
                } else {
                    write!(f, "{}::{}(", enum_name, variant)?;
                    for (i, v) in fields.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", v)?;
                    }
                    write!(f, ")")
                }
            }
            Value::Poll(ready, val) => {
                if *ready {
                    if let Some(v) = val {
                        write!(f, "Poll::Ready({})", v)
                    } else {
                        write!(f, "Poll::Ready(())")
                    }
                } else {
                    write!(f, "Poll::Pending")
                }
            }
            Value::Future(name, _) => write!(f, "<future {}>", name),
            Value::Break(v) => {
                if let Some(val) = v {
                    write!(f, "<break {}>", val)
                } else {
                    write!(f, "<break>")
                }
            }
            Value::Continue => write!(f, "<continue>"),
        }
    }
}

/// Reference to an actor
#[derive(Debug, Clone)]
pub struct ActorRef {
    pub id: u64,
    pub sender: Sender<Message>,
}

/// Message for actor communication
#[derive(Debug, Clone)]
pub struct Message {
    pub name: String,
    pub args: Vec<Value>,
}

/// Interpreter environment
#[derive(Clone)]
pub struct Env {
    scopes: Vec<HashMap<String, Value>>,
    functions: HashMap<String, Function>,
    components: HashMap<String, Component>,
    /// Methods: type_name -> method_name -> function
    methods: HashMap<String, HashMap<String, Function>>,
    #[allow(dead_code)]
    actors: HashMap<u64, Sender<Message>>,
    #[allow(dead_code)]
    next_actor_id: u64,
    actor_defs: HashMap<String, Actor>,
    /// ID of the current actor if running inside one
    self_actor_id: Option<u64>,
    /// Python global scope
    python_scope: Option<PyObject>,
}

impl Env {
    pub fn new() -> Self {
        let mut env = Self {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
            components: HashMap::new(),
            methods: HashMap::new(),
            actors: HashMap::new(),
            next_actor_id: 1,
            actor_defs: HashMap::new(),
            self_actor_id: None,
            python_scope: None,
        };

        // Initialize Python scope
        Python::with_gil(|py| {
            let locals = PyDict::new(py);
            env.python_scope = Some(locals.into());
        });

        env.register_stdlib();
        env.register_net_stdlib();
        env.register_stdlib();
        env.register_net_stdlib();
        env.register_json_stdlib();
        env.register_kos_bridge();
        env
    }

    pub fn register_kos_bridge(&mut self) {
        self.define_native("spawn_cube", |_env, args| {
            if args.len() != 2 {
                return Err(KainError::runtime(
                    "spawn_cube: expected 2 arguments (x, y)",
                ));
            }
            let x = match args[0] {
                Value::Int(n) => n as f64,
                Value::Float(n) => n,
                _ => return Err(KainError::runtime("spawn_cube: x must be number")),
            };
            let y = match args[1] {
                Value::Int(n) => n as f64,
                Value::Float(n) => n,
                _ => return Err(KainError::runtime("spawn_cube: y must be number")),
            };

            println!(
                " [KOS Bridge] Spawning Cube at {{ x: {:.2}, y: {:.2} }}",
                x, y
            );
            Ok(Value::Unit)
        });
    }

    pub fn register_net_stdlib(&mut self) {
        // === HTTP Operations ===
        self.define_native("http_get", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("http_get: expected 1 argument (url)"));
            }
            let url = match &args[0] {
                Value::String(s) => s.clone(),
                _ => return Err(KainError::runtime("http_get: argument must be string url")),
            };

            let res = reqwest::blocking::get(&url);

            match res {
                Ok(resp) => match resp.text() {
                    Ok(text) => Ok(Value::String(text)),
                    Err(e) => Err(KainError::runtime(format!(
                        "http_get: failed to read body: {}",
                        e
                    ))),
                },
                Err(e) => Err(KainError::runtime(format!(
                    "http_get: request failed: {}",
                    e
                ))),
            }
        });

        self.define_native("http_post_json", |_env, args| {
            if args.len() != 2 {
                return Err(KainError::runtime(
                    "http_post: expected 2 arguments (url, json_string)",
                ));
            }
            let url = match &args[0] {
                Value::String(s) => s.clone(),
                _ => return Err(KainError::runtime("http_post: url must be string")),
            };
            let body = match &args[1] {
                Value::String(s) => s.clone(),
                _ => return Err(KainError::runtime("http_post: body must be string")),
            };

            let client = reqwest::blocking::Client::new();

            let res = client
                .post(&url)
                .header("Content-Type", "application/json")
                .body(body)
                .send();

            match res {
                Ok(resp) => match resp.text() {
                    Ok(text) => Ok(Value::String(text)),
                    Err(e) => Err(KainError::runtime(format!(
                        "http_post: failed to read response: {}",
                        e
                    ))),
                },
                Err(e) => Err(KainError::runtime(format!(
                    "http_post: request failed: {}",
                    e
                ))),
            }
        });
    }

    pub fn register_json_stdlib(&mut self) {
        self.define_native("json_parse", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime(
                    "json_parse: expected 1 argument (string)",
                ));
            }
            let s = match &args[0] {
                Value::String(s) => s,
                _ => return Err(KainError::runtime("json_parse: argument must be string")),
            };

            fn from_json(v: &serde_json::Value) -> Value {
                match v {
                    serde_json::Value::Null => Value::None,
                    serde_json::Value::Bool(b) => Value::Bool(*b),
                    serde_json::Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            Value::Int(i)
                        } else if let Some(f) = n.as_f64() {
                            Value::Float(f)
                        } else {
                            Value::Int(0) // Should match
                        }
                    }
                    serde_json::Value::String(s) => Value::String(s.clone()),
                    serde_json::Value::Array(arr) => {
                        let k_arr = arr.iter().map(from_json).collect();
                        Value::Array(Arc::new(RwLock::new(k_arr)))
                    }
                    serde_json::Value::Object(obj) => {
                        let mut map = HashMap::new();
                        for (k, v) in obj {
                            map.insert(k.clone(), from_json(v));
                        }
                        Value::Struct("Json".to_string(), Arc::new(RwLock::new(map)))
                    }
                }
            }

            match serde_json::from_str::<serde_json::Value>(s) {
                Ok(v) => Ok(from_json(&v)),
                Err(e) => Err(KainError::runtime(format!(
                    "json_parse: invalid json: {}",
                    e
                ))),
            }
        });

        self.define_native("json_string", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("json_string: expected 1 argument"));
            }

            fn to_json(v: &Value) -> serde_json::Value {
                match v {
                    Value::Unit => serde_json::Value::Null,
                    Value::None => serde_json::Value::Null,
                    Value::Bool(b) => serde_json::Value::Bool(*b),
                    Value::Int(i) => serde_json::json!(i),
                    Value::Float(f) => serde_json::json!(f),
                    Value::String(s) => serde_json::Value::String(s.clone()),
                    Value::Array(arr) => {
                        let arr = arr.read().unwrap();
                        serde_json::Value::Array(arr.iter().map(to_json).collect())
                    }
                    Value::Struct(_, fields) => {
                        let fields = fields.read().unwrap();
                        let mut map = serde_json::Map::new();
                        for (k, v) in fields.iter() {
                            map.insert(k.clone(), to_json(v));
                        }
                        serde_json::Value::Object(map)
                    }
                    Value::Tuple(items) => {
                        serde_json::Value::Array(items.iter().map(to_json).collect())
                    }
                    _ => serde_json::Value::String(format!("{}", v)), // Fallback
                }
            }

            Ok(Value::String(to_json(&args[0]).to_string()))
        });
    }

    pub fn register_stdlib(&mut self) {
        // Register built-in constants
        self.define("None".to_string(), Value::None);
        self.define("none".to_string(), Value::None); // Also lowercase for convenience

        // Some is just an identity function - returns its argument
        // This lets code use Some(value) pattern even though we don't have proper Option types
        self.define_native("Some", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("Some: expected 1 argument"));
            }
            Ok(args[0].clone())
        });

        // Register built-in functions
        self.define_native("print", |_env, args| {
            for arg in args {
                print!("{} ", arg);
            }
            Ok(Value::Unit)
        });

        self.define_native("println", |_env, args| {
            for arg in args {
                print!("{} ", arg);
            }
            println!("");
            Ok(Value::Unit)
        });

        // Math functions
        self.define_native("min", |_env, args| {
            if args.len() != 2 {
                return Err(KainError::runtime("min: expected 2 arguments"));
            }
            match (&args[0], &args[1]) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(*a.min(b))),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.min(*b))),
                _ => Err(KainError::runtime("min: arguments must be numbers")),
            }
        });

        self.define_native("max", |_env, args| {
            if args.len() != 2 {
                return Err(KainError::runtime("max: expected 2 arguments"));
            }
            match (&args[0], &args[1]) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(*a.max(b))),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.max(*b))),
                _ => Err(KainError::runtime("max: arguments must be numbers")),
            }
        });

        self.define_native("abs", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("abs: expected 1 argument"));
            }
            match &args[0] {
                Value::Int(n) => Ok(Value::Int(n.abs())),
                Value::Float(n) => Ok(Value::Float(n.abs())),
                _ => Err(KainError::runtime("abs: argument must be number")),
            }
        });

        self.define_native("sqrt", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("sqrt: expected 1 argument"));
            }
            match &args[0] {
                Value::Int(n) => Ok(Value::Float((*n as f64).sqrt())),
                Value::Float(n) => Ok(Value::Float(n.sqrt())),
                _ => Err(KainError::runtime("sqrt: argument must be number")),
            }
        });

        // Random
        self.define_native("random", |_env, _args| {
            // Simple LCG for deterministic behavior in prototype
            // In real impl use rand crate
            use std::time::SystemTime;
            let seed = SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64;
            let x = (seed % 1000) as f64 / 1000.0;
            Ok(Value::Float(x))
        });

        self.define_native("sleep", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("sleep: expected 1 argument (ms)"));
            }
            match args[0] {
                Value::Int(ms) => {
                    std::thread::sleep(std::time::Duration::from_millis(ms as u64));
                    Ok(Value::Unit)
                }
                _ => Err(KainError::runtime("sleep: argument must be int")),
            }
        });

        // Collections
        self.define_native("len", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("len: expected 1 argument"));
            }
            match &args[0] {
                Value::String(s) => Ok(Value::Int(s.len() as i64)),
                Value::Array(arr) => Ok(Value::Int(arr.read().unwrap().len() as i64)),
                _ => Err(KainError::runtime("len: argument must be string or array")),
            }
        });

        // ord: get ASCII/Unicode code of first character
        self.define_native("ord", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("ord: expected 1 argument"));
            }
            match &args[0] {
                Value::String(s) => {
                    if let Some(c) = s.chars().next() {
                        Ok(Value::Int(c as i64))
                    } else {
                        Err(KainError::runtime("ord: empty string"))
                    }
                }
                _ => Err(KainError::runtime("ord: argument must be string")),
            }
        });

        // chr: convert code to character
        self.define_native("chr", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("chr: expected 1 argument"));
            }
            match &args[0] {
                Value::Int(n) => {
                    if let Some(c) = char::from_u32(*n as u32) {
                        Ok(Value::String(c.to_string()))
                    } else {
                        Err(KainError::runtime("chr: invalid code point"))
                    }
                }
                _ => Err(KainError::runtime("chr: argument must be int")),
            }
        });

        self.define_native("first", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("first: expected 1 argument"));
            }
            match &args[0] {
                Value::Array(arr) => {
                    let arr = arr.read().unwrap();
                    if arr.is_empty() {
                        return Err(KainError::runtime("first: empty array"));
                    }
                    Ok(arr[0].clone())
                }
                _ => Err(KainError::runtime("first: argument must be array")),
            }
        });

        self.define_native("last", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("last: expected 1 argument"));
            }
            match &args[0] {
                Value::Array(arr) => {
                    let arr = arr.read().unwrap();
                    if arr.is_empty() {
                        return Err(KainError::runtime("last: empty array"));
                    }
                    Ok(arr[arr.len() - 1].clone())
                }
                _ => Err(KainError::runtime("last: argument must be array")),
            }
        });

        self.define_native("push", |_env, args| {
            if args.len() != 2 {
                return Err(KainError::runtime("push: expected 2 arguments"));
            }
            match &args[0] {
                Value::Array(arr) => {
                    arr.write().unwrap().push(args[1].clone());
                    Ok(Value::Unit)
                }
                _ => Err(KainError::runtime("push: first argument must be array")),
            }
        });

        // Range
        self.define_native("range", |_env, args| {
            if args.len() != 2 {
                return Err(KainError::runtime("range: expected 2 arguments"));
            }
            let start = match args[0] {
                Value::Int(n) => n,
                _ => return Err(KainError::runtime("range: expected int")),
            };
            let end = match args[1] {
                Value::Int(n) => n,
                _ => return Err(KainError::runtime("range: expected int")),
            };

            let arr = (start..end).map(Value::Int).collect();
            Ok(Value::Array(Arc::new(RwLock::new(arr))))
        });

        // Array Utils
        self.define_native("first", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("first: expected 1 argument"));
            }
            match &args[0] {
                Value::Array(arr) => arr
                    .read()
                    .unwrap()
                    .first()
                    .cloned()
                    .ok_or_else(|| KainError::runtime("Array is empty")),
                Value::String(s) => s
                    .chars()
                    .next()
                    .map(|c| Value::String(c.to_string()))
                    .ok_or_else(|| KainError::runtime("String is empty")),
                _ => Err(KainError::runtime("first: expected array or string")),
            }
        });

        self.define_native("last", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("last: expected 1 argument"));
            }
            match &args[0] {
                Value::Array(arr) => arr
                    .read()
                    .unwrap()
                    .last()
                    .cloned()
                    .ok_or_else(|| KainError::runtime("Array is empty")),
                Value::String(s) => s
                    .chars()
                    .last()
                    .map(|c| Value::String(c.to_string()))
                    .ok_or_else(|| KainError::runtime("String is empty")),
                _ => Err(KainError::runtime("last: expected array or string")),
            }
        });

        self.define_native("reverse", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("reverse: expected 1 argument"));
            }
            match &args[0] {
                Value::Array(arr) => {
                    let mut reversed = arr.read().unwrap().clone();
                    reversed.reverse();
                    Ok(Value::Array(Arc::new(RwLock::new(reversed))))
                }
                Value::String(s) => Ok(Value::String(s.chars().rev().collect())),
                _ => Err(KainError::runtime("reverse: expected array or string")),
            }
        });

        self.define_native("sum", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("sum: expected 1 argument"));
            }
            match &args[0] {
                Value::Array(arr) => {
                    let mut total = 0i64;
                    for v in arr.read().unwrap().iter() {
                        match v {
                            Value::Int(n) => total += n,
                            _ => {
                                return Err(KainError::runtime("sum: array must contain integers"))
                            }
                        }
                    }
                    Ok(Value::Int(total))
                }
                _ => Err(KainError::runtime("sum: expected array")),
            }
        });

        // === Type checks ===
        self.define_native("type_of", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("type_of: expected 1 argument"));
            }
            let type_name = match &args[0] {
                Value::Unit => "unit",
                Value::Bool(_) => "bool",
                Value::Int(_) => "int",
                Value::Float(_) => "float",
                Value::String(_) => "string",
                Value::Array(_) => "array",
                Value::Tuple(_) => "tuple",
                Value::Struct(name, _) => name.as_str(),
                Value::Function(_) => "function",
                Value::NativeFn(_, _) => "native_function",
                Value::ActorRef(_) => "actor",
                Value::None => "none",
                Value::Return(_) => "return_value",
                Value::Closure(_, _, _) => "function",
                Value::Result(_, _) => "result",
                Value::StructConstructor(_, _) => "struct_constructor",
                Value::JSX(_) => "jsx",
                Value::EnumVariant(enum_name, _, _) => return Ok(Value::String(enum_name.clone())),
                Value::Poll(_, _) => "poll",
                Value::Future(name, _) => return Ok(Value::String(format!("Future<{}>", name))),
                Value::Break(_) => "break",
                Value::Continue => "continue",
            };
            Ok(Value::String(type_name.to_string()))
        });

        // Get the variant name of an enum (e.g., "Int" from Expr::Int(42))
        self.define_native("variant_of", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("variant_of: expected 1 argument"));
            }
            match &args[0] {
                Value::EnumVariant(_, variant, _) => Ok(Value::String(variant.clone())),
                _ => Ok(Value::String("".to_string())), // Not an enum variant
            }
        });

        // Get a field from an enum variant by index (0-based)
        // Example: variant_field(Expr::Binary(left, op, right), 0) returns left
        self.define_native("variant_field", |_env, args| {
            if args.len() != 2 {
                return Err(KainError::runtime(
                    "variant_field: expected 2 arguments (enum, index)",
                ));
            }
            let idx = match &args[1] {
                Value::Int(n) => *n as usize,
                _ => return Err(KainError::runtime("variant_field: index must be int")),
            };
            match &args[0] {
                Value::EnumVariant(_, _, fields) => {
                    if idx < fields.len() {
                        let field = fields[idx].clone();
                        // Auto-unwrap Box values (Struct "Box" with field "0")
                        if let Value::Struct(name, inner) = &field {
                            if name == "Box" {
                                let inner = inner.read().unwrap();
                                if let Some(boxed) = inner.get("0") {
                                    return Ok(boxed.clone());
                                }
                            }
                        }
                        // Auto-unwrap Box::new(...) pattern (EnumVariant "Box" / "new")
                        if let Value::EnumVariant(enum_name, variant_name, inner_fields) = &field {
                            if enum_name == "Box"
                                && variant_name == "new"
                                && inner_fields.len() == 1
                            {
                                return Ok(inner_fields[0].clone());
                            }
                        }
                        Ok(field)
                    } else {
                        Err(KainError::runtime(format!(
                            "variant_field: index {} out of bounds (has {} fields)",
                            idx,
                            fields.len()
                        )))
                    }
                }
                _ => Err(KainError::runtime(
                    "variant_field: first argument must be enum variant",
                )),
            }
        });

        self.define_native("str", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("str: expected 1 argument"));
            }
            Ok(Value::String(format!("{}", args[0])))
        });

        self.define_native("int", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("int: expected 1 argument"));
            }
            match &args[0] {
                Value::Int(n) => Ok(Value::Int(*n)),
                Value::Float(n) => Ok(Value::Int(*n as i64)),
                Value::String(s) => s
                    .parse::<i64>()
                    .map(Value::Int)
                    .map_err(|_| KainError::runtime("Invalid int string")),
                _ => Err(KainError::runtime("int: argument must be number or string")),
            }
        });

        self.define_native("float", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("float: expected 1 argument"));
            }
            match &args[0] {
                Value::Int(n) => Ok(Value::Float(*n as f64)),
                Value::Float(n) => Ok(Value::Float(*n)),
                Value::String(s) => s
                    .parse::<f64>()
                    .map(Value::Float)
                    .map_err(|_| KainError::runtime("Invalid float string")),
                _ => Err(KainError::runtime(
                    "float: argument must be number or string",
                )),
            }
        });

        // === Result / Error Handling ===
        self.define_native("ok", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("ok: expected 1 argument"));
            }
            Ok(Value::Result(true, Box::new(args[0].clone())))
        });

        self.define_native("err", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("err: expected 1 argument"));
            }
            Ok(Value::Result(false, Box::new(args[0].clone())))
        });

        self.define_native("sleep", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("sleep: expected 1 argument"));
            }
            match &args[0] {
                Value::Int(n) => {
                    std::thread::sleep(std::time::Duration::from_secs(*n as u64));
                    Ok(Value::Unit)
                }
                Value::Float(n) => {
                    std::thread::sleep(std::time::Duration::from_secs_f64(*n));
                    Ok(Value::Unit)
                }
                _ => Err(KainError::runtime("sleep: expected number")),
            }
        });

        self.define_native("now", |_env, _args| {
            let start = std::time::SystemTime::now();
            let since_the_epoch = start
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| KainError::runtime(&format!("Time error: {}", e)))?;
            Ok(Value::Float(since_the_epoch.as_secs_f64()))
        });

        // === Higher-Order Functions ===
        // Note: These need special handling since they take closures
        // We'll register them but they need to be called via call_function
        self.define_native("map", |env, args| {
            if args.len() != 2 {
                return Err(KainError::runtime(
                    "map: expected 2 arguments (array, function)",
                ));
            }
            let arr = match &args[0] {
                Value::Array(a) => a.read().unwrap().clone(),
                _ => return Err(KainError::runtime("map: first argument must be an array")),
            };
            let func = args[1].clone();
            let mut results = Vec::new();
            for item in arr {
                let result = call_function(env, func.clone(), vec![item])?;
                results.push(result);
            }
            Ok(Value::Array(Arc::new(RwLock::new(results))))
        });

        self.define_native("filter", |env, args| {
            if args.len() != 2 {
                return Err(KainError::runtime(
                    "filter: expected 2 arguments (array, function)",
                ));
            }
            let arr = match &args[0] {
                Value::Array(a) => a.read().unwrap().clone(),
                _ => {
                    return Err(KainError::runtime(
                        "filter: first argument must be an array",
                    ))
                }
            };
            let func = args[1].clone();
            let mut results = Vec::new();
            for item in arr {
                let result = call_function(env, func.clone(), vec![item.clone()])?;
                match result {
                    Value::Bool(true) => results.push(item),
                    Value::Bool(false) => {}
                    _ => return Err(KainError::runtime("filter: function must return bool")),
                }
            }
            Ok(Value::Array(Arc::new(RwLock::new(results))))
        });

        self.define_native("reduce", |env, args| {
            if args.len() != 3 {
                return Err(KainError::runtime(
                    "reduce: expected 3 arguments (array, initial, function)",
                ));
            }
            let arr = match &args[0] {
                Value::Array(a) => a.read().unwrap().clone(),
                _ => {
                    return Err(KainError::runtime(
                        "reduce: first argument must be an array",
                    ))
                }
            };
            let mut acc = args[1].clone();
            let func = args[2].clone();
            for item in arr {
                acc = call_function(env, func.clone(), vec![acc, item])?;
            }
            Ok(acc)
        });

        self.define_native("foreach", |env, args| {
            if args.len() != 2 {
                return Err(KainError::runtime(
                    "foreach: expected 2 arguments (array, function)",
                ));
            }
            let arr = match &args[0] {
                Value::Array(a) => a.read().unwrap().clone(),
                _ => {
                    return Err(KainError::runtime(
                        "foreach: first argument must be an array",
                    ))
                }
            };
            let func = args[1].clone();
            for item in arr {
                call_function(env, func.clone(), vec![item])?;
            }
            Ok(Value::Unit)
        });

        // === File I/O ===
        self.define_native("read_file", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("read_file: expected 1 argument"));
            }
            let path = match &args[0] {
                Value::String(s) => s,
                _ => return Err(KainError::runtime("read_file: argument must be string")),
            };

            match std::fs::read_to_string(path) {
                Ok(s) => Ok(Value::String(s)),
                Err(e) => Ok(Value::Result(
                    false,
                    Box::new(Value::String(format!("Failed to read file: {}", e))),
                )),
            }
        });

        self.define_native("write_file", |_env, args| {
            if args.len() != 2 {
                return Err(KainError::runtime("write_file: expected 2 arguments"));
            }
            let path = match &args[0] {
                Value::String(s) => s,
                _ => {
                    return Err(KainError::runtime(
                        "write_file: first argument must be string",
                    ))
                }
            };
            let content = match &args[1] {
                Value::String(s) => s,
                _ => {
                    return Err(KainError::runtime(
                        "write_file: second argument must be string",
                    ))
                }
            };

            match std::fs::write(path, content) {
                Ok(_) => Ok(Value::Unit),
                Err(e) => Ok(Value::Result(
                    false,
                    Box::new(Value::String(format!("Failed to read file: {}", e))),
                )),
            }
        });

        // === String Functions ===
        self.define_native("split", |_env, args| {
            if args.len() != 2 {
                return Err(KainError::runtime(
                    "split: expected 2 arguments (string, delimiter)",
                ));
            }
            let s = match &args[0] {
                Value::String(s) => s.clone(),
                _ => return Err(KainError::runtime("split: first argument must be a string")),
            };
            let delim = match &args[1] {
                Value::String(s) => s.clone(),
                _ => {
                    return Err(KainError::runtime(
                        "split: second argument must be a string",
                    ))
                }
            };
            // Handle empty delimiter specially - split into individual characters
            let parts: Vec<Value> = if delim.is_empty() {
                s.chars().map(|c| Value::String(c.to_string())).collect()
            } else {
                s.split(&delim)
                    .map(|p| Value::String(p.to_string()))
                    .collect()
            };
            Ok(Value::Array(Arc::new(RwLock::new(parts))))
        });

        self.define_native("join", |_env, args| {
            if args.len() != 2 {
                return Err(KainError::runtime(
                    "join: expected 2 arguments (array, delimiter)",
                ));
            }
            let arr = match &args[0] {
                Value::Array(a) => a.read().unwrap().clone(),
                _ => return Err(KainError::runtime("join: first argument must be an array")),
            };
            let delim = match &args[1] {
                Value::String(s) => s.clone(),
                _ => return Err(KainError::runtime("join: second argument must be a string")),
            };
            let parts: Vec<String> = arr.iter().map(|v| format!("{}", v)).collect();
            Ok(Value::String(parts.join(&delim)))
        });

        self.define_native("trim", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("trim: expected 1 argument (string)"));
            }
            match &args[0] {
                Value::String(s) => Ok(Value::String(s.trim().to_string())),
                _ => Err(KainError::runtime("trim: argument must be a string")),
            }
        });

        self.define_native("upper", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("upper: expected 1 argument (string)"));
            }
            match &args[0] {
                Value::String(s) => Ok(Value::String(s.to_uppercase())),
                _ => Err(KainError::runtime("upper: argument must be a string")),
            }
        });

        self.define_native("lower", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("lower: expected 1 argument (string)"));
            }
            match &args[0] {
                Value::String(s) => Ok(Value::String(s.to_lowercase())),
                _ => Err(KainError::runtime("lower: argument must be a string")),
            }
        });

        self.define_native("contains", |_env, args| {
            if args.len() != 2 {
                return Err(KainError::runtime(
                    "contains: expected 2 arguments (string, pattern)",
                ));
            }
            let s = match &args[0] {
                Value::String(s) => s.clone(),
                Value::Array(arr) => {
                    // Support array.contains(element) for various types
                    let needle = &args[1];
                    return Ok(Value::Bool(arr.read().unwrap().iter().any(|v| {
                        match (v, needle) {
                            (Value::Int(n1), Value::Int(n2)) => n1 == n2,
                            (Value::String(s1), Value::String(s2)) => s1 == s2,
                            (Value::Bool(b1), Value::Bool(b2)) => b1 == b2,
                            _ => false,
                        }
                    })));
                }
                _ => {
                    return Err(KainError::runtime(
                        "contains: first argument must be a string or array",
                    ))
                }
            };
            let sub = match &args[1] {
                Value::String(s) => s.clone(),
                _ => {
                    return Err(KainError::runtime(
                        "contains: second argument must be a string",
                    ))
                }
            };
            Ok(Value::Bool(s.contains(&sub)))
        });

        self.define_native("starts_with", |_env, args| {
            if args.len() != 2 {
                return Err(KainError::runtime("starts_with: expected 2 arguments"));
            }
            let s = match &args[0] {
                Value::String(s) => s,
                _ => return Err(KainError::runtime("expected string")),
            };
            let sub = match &args[1] {
                Value::String(s) => s,
                _ => return Err(KainError::runtime("expected string")),
            };
            Ok(Value::Bool(s.starts_with(sub)))
        });

        self.define_native("ends_with", |_env, args| {
            if args.len() != 2 {
                return Err(KainError::runtime("ends_with: expected 2 arguments"));
            }
            let s = match &args[0] {
                Value::String(s) => s,
                _ => return Err(KainError::runtime("expected string")),
            };
            let sub = match &args[1] {
                Value::String(s) => s,
                _ => return Err(KainError::runtime("expected string")),
            };
            Ok(Value::Bool(s.ends_with(sub)))
        });

        self.define_native("replace", |_env, args| {
            if args.len() != 3 {
                return Err(KainError::runtime(
                    "replace: expected 3 arguments (string, from, to)",
                ));
            }
            let s = match &args[0] {
                Value::String(s) => s,
                _ => return Err(KainError::runtime("expected string")),
            };
            let from = match &args[1] {
                Value::String(s) => s,
                _ => return Err(KainError::runtime("expected string")),
            };
            let to = match &args[2] {
                Value::String(s) => s,
                _ => return Err(KainError::runtime("expected string")),
            };
            Ok(Value::String(s.replace(from, to)))
        });

        self.define_native("char_at", |_env, args| {
            if args.len() != 2 {
                return Err(KainError::runtime("char_at: expected 2 arguments"));
            }
            let s = match &args[0] {
                Value::String(s) => s,
                _ => return Err(KainError::runtime("expected string")),
            };
            let idx = match &args[1] {
                Value::Int(n) => *n as usize,
                _ => return Err(KainError::runtime("expected int")),
            };
            match s.chars().nth(idx) {
                Some(c) => Ok(Value::String(c.to_string())),
                None => Ok(Value::None),
            }
        });

        self.define_native("substring", |_env, args| {
            if args.len() < 2 || args.len() > 3 {
                return Err(KainError::runtime(
                    "substring: expected 2-3 arguments (string, start, [end])",
                ));
            }
            let s = match &args[0] {
                Value::String(s) => s.clone(),
                _ => {
                    return Err(KainError::runtime(
                        "substring: first argument must be a string",
                    ))
                }
            };
            let start = match &args[1] {
                Value::Int(n) => *n as usize,
                _ => {
                    return Err(KainError::runtime(
                        "substring: second argument must be an integer",
                    ))
                }
            };
            let end = if args.len() == 3 {
                match &args[2] {
                    Value::Int(n) => *n as usize,
                    _ => {
                        return Err(KainError::runtime(
                            "substring: third argument must be an integer",
                        ))
                    }
                }
            } else {
                s.len()
            };
            let chars: String = s.chars().skip(start).take(end - start).collect();
            Ok(Value::String(chars))
        });

        // === Actor System ===

        self.define_native("send", |_env, args| {
            if args.len() < 2 {
                return Err(KainError::runtime(
                    "send: expected at least 2 arguments (actor, msg_name)",
                ));
            }
            let actor_ref = match &args[0] {
                Value::ActorRef(r) => r,
                _ => return Err(KainError::runtime("send: first argument must be actor ref")),
            };
            let msg_name = match &args[1] {
                Value::String(s) => s.clone(),
                _ => {
                    return Err(KainError::runtime(
                        "send: second argument must be message name",
                    ))
                }
            };

            let msg_args = args[2..].to_vec();

            let _ = actor_ref.sender.send(Message {
                name: msg_name,
                args: msg_args,
            });

            Ok(Value::Unit)
        });

        self.define_native("sleep", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("sleep: expected 1 argument (ms)"));
            }
            let ms = match args[0] {
                Value::Int(i) => i as u64,
                _ => return Err(KainError::runtime("sleep: expected int")),
            };
            std::thread::sleep(std::time::Duration::from_millis(ms));
            Ok(Value::Unit)
        });

        // === Utility Functions ===
        self.define_native("time", |_env, _args| {
            use std::time::{SystemTime, UNIX_EPOCH};
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            Ok(Value::Float(now.as_secs_f64()))
        });

        self.define_native("exit", |_env, args| {
            let code = if args.len() > 0 {
                match args[0] {
                    Value::Int(n) => n as i32,
                    _ => 0,
                }
            } else {
                0
            };
            std::process::exit(code);
        });

        self.define_native("env", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("env: expected 1 argument"));
            }
            match &args[0] {
                Value::String(key) => match std::env::var(key) {
                    Ok(v) => Ok(Value::String(v)),
                    Err(_) => Ok(Value::None),
                },
                _ => Err(KainError::runtime("env: expected string key")),
            }
        });

        self.define_native("assert", |_env, args| {
            if args.len() < 1 {
                return Err(KainError::runtime("assert: expected condition"));
            }
            match &args[0] {
                Value::Bool(true) => Ok(Value::Unit),
                _ => {
                    let msg = if args.len() > 1 {
                        format!("{}", args[1])
                    } else {
                        "Assertion failed".to_string()
                    };
                    Err(KainError::runtime(msg))
                }
            }
        });

        self.define_native("panic", |_env, args| {
            let msg = if args.len() > 0 {
                format!("{}", args[0])
            } else {
                "Panic".to_string()
            };
            Err(KainError::runtime(msg))
        });

        // Debug
        self.define_native("dbg", |_env, args| {
            for arg in args {
                println!("[DEBUG] {:?}", arg);
            }
            Ok(Value::Unit)
        });

        // Conversion
        self.define_native("int", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("int: expected 1 argument"));
            }
            match &args[0] {
                Value::Int(n) => Ok(Value::Int(*n)),
                Value::Float(n) => Ok(Value::Int(*n as i64)),
                Value::String(s) => s
                    .parse::<i64>()
                    .map(Value::Int)
                    .map_err(|_| KainError::runtime(format!("Cannot parse '{}' as int", s))),
                Value::Bool(b) => Ok(Value::Int(if *b { 1 } else { 0 })),
                _ => Err(KainError::runtime("int: cannot convert this type")),
            }
        });

        self.define_native("float", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("float: expected 1 argument"));
            }
            match &args[0] {
                Value::Int(n) => Ok(Value::Float(*n as f64)),
                Value::Float(n) => Ok(Value::Float(*n)),
                Value::String(s) => s
                    .parse::<f64>()
                    .map(Value::Float)
                    .map_err(|_| KainError::runtime(format!("Cannot parse '{}' as float", s))),
                _ => Err(KainError::runtime("float: cannot convert this type")),
            }
        });

        self.define_native("str", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("str: expected 1 argument"));
            }
            Ok(Value::String(format!("{}", &args[0])))
        });

        // Alias for str
        self.define_native("to_string", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("to_string: expected 1 argument"));
            }
            Ok(Value::String(format!("{}", &args[0])))
        });

        self.define_native("bool", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("bool: expected 1 argument"));
            }
            let result = match &args[0] {
                Value::Bool(b) => *b,
                Value::Int(n) => *n != 0,
                Value::Float(n) => *n != 0.0,
                Value::String(s) => !s.is_empty(),
                Value::None => false,
                Value::Unit => false,
                _ => true,
            };
            Ok(Value::Bool(result))
        });

        // Legacy helpers
        self.define_native("to_int", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("to_int: expected 1 argument"));
            }
            match &args[0] {
                Value::Int(n) => Ok(Value::Int(*n)),
                Value::Float(n) => Ok(Value::Int(*n as i64)),
                Value::String(s) => s
                    .parse::<i64>()
                    .map(Value::Int)
                    .map_err(|_| KainError::runtime(format!("Cannot parse '{}' as int", s))),
                Value::Bool(b) => Ok(Value::Int(if *b { 1 } else { 0 })),
                _ => Err(KainError::runtime("to_int: cannot convert this type")),
            }
        });

        // === Math ===
        self.define_native("sqrt", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("sqrt: expected 1 argument"));
            }
            match args[0] {
                Value::Int(n) => Ok(Value::Float((n as f64).sqrt())),
                Value::Float(n) => Ok(Value::Float(n.sqrt())),
                _ => Err(KainError::runtime("sqrt: expected number")),
            }
        });

        self.define_native("sin", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("sin: expected 1 argument"));
            }
            match args[0] {
                Value::Int(n) => Ok(Value::Float((n as f64).sin())),
                Value::Float(n) => Ok(Value::Float(n.sin())),
                _ => Err(KainError::runtime("sin: expected number")),
            }
        });

        self.define_native("cos", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("cos: expected 1 argument"));
            }
            match args[0] {
                Value::Int(n) => Ok(Value::Float((n as f64).cos())),
                Value::Float(n) => Ok(Value::Float(n.cos())),
                _ => Err(KainError::runtime("cos: expected number")),
            }
        });

        self.define_native("tan", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("tan: expected 1 argument"));
            }
            match args[0] {
                Value::Int(n) => Ok(Value::Float((n as f64).tan())),
                Value::Float(n) => Ok(Value::Float(n.tan())),
                _ => Err(KainError::runtime("tan: expected number")),
            }
        });

        // === I/O ===
        self.define_native("read_line", |_env, _args| {
            use std::io::{self, BufRead};
            let stdin = io::stdin();
            let mut line = String::new();
            stdin.lock().read_line(&mut line).ok();
            Ok(Value::String(line.trim_end().to_string()))
        });

        // Python FFI
        self.define_native("py_eval", |env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("py_eval: expected 1 argument (code)"));
            }
            let code = match &args[0] {
                Value::String(s) => s,
                _ => return Err(KainError::runtime("py_eval: expected string")),
            };

            let scope = env.python_scope.as_ref().unwrap();

            Python::with_gil(|py| {
                let locals = scope.as_ref(py).downcast::<PyDict>().unwrap();
                let result = py
                    .eval(code, None, Some(locals))
                    .map_err(|e| KainError::runtime(format!("Python Error: {}", e)))?;
                py_to_value(result)
                    .map_err(|e| KainError::runtime(format!("Conversion Error: {}", e)))
            })
        });

        self.define_native("py_exec", |env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("py_exec: expected 1 argument"));
            }
            let code = match &args[0] {
                Value::String(s) => s,
                _ => return Err(KainError::runtime("py_exec: expected string")),
            };

            let scope = env.python_scope.as_ref().unwrap();

            Python::with_gil(|py| {
                let locals = scope.as_ref(py).downcast::<PyDict>().unwrap();
                py.run(code, None, Some(locals))
                    .map_err(|e| KainError::runtime(format!("Python Error: {}", e)))?;
                Ok(Value::Unit)
            })
        });

        self.define_native("py_import", |env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("py_import: expected 1 argument"));
            }
            let module_name = match &args[0] {
                Value::String(s) => s,
                _ => return Err(KainError::runtime("py_import: argument must be string")),
            };

            let scope = env.python_scope.as_ref().unwrap();

            Python::with_gil(|py| {
                let locals = scope.as_ref(py).downcast::<PyDict>().unwrap();
                let module = py
                    .import(module_name.as_str())
                    .map_err(|e| KainError::runtime(format!("Python error: {}", e)))?;

                // Add module to locals with its name
                locals
                    .set_item(module_name, module)
                    .map_err(|e| KainError::runtime(format!("Failed to set module: {}", e)))?;

                py_to_value(module)
                    .map_err(|e| KainError::runtime(format!("Conversion Error: {}", e)))
            })
        });

        self.define_native("file_exists", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("file_exists: expected 1 argument"));
            }
            match &args[0] {
                Value::String(path) => Ok(Value::Bool(std::path::Path::new(path).exists())),
                _ => Err(KainError::runtime("file_exists: path must be string")),
            }
        });

        // === ASYNC RUNTIME ===

        // block_on: Run a future to completion, blocking the current thread
        self.define_native("block_on", |env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("block_on: expected 1 argument (future)"));
            }

            let future_val = args[0].clone();
            poll_future_to_completion(env, future_val)
        });

        // spawn_task: Spawn an async task (runs it immediately in this simple executor)
        self.define_native("spawn_task", |env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime(
                    "spawn_task: expected 1 argument (future)",
                ));
            }

            // For this simple executor, spawn is just block_on
            // A real executor would add to a task queue
            let future_val = args[0].clone();
            poll_future_to_completion(env, future_val)
        });

        // poll_once: Poll a future once and return the Poll result
        self.define_native("poll_once", |env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime(
                    "poll_once: expected 1 argument (future)",
                ));
            }

            poll_future_once(env, args[0].clone())
        });

        // is_ready: Check if a Poll value is Ready
        self.define_native("is_ready", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("is_ready: expected 1 argument"));
            }

            match &args[0] {
                Value::Poll(ready, _) => Ok(Value::Bool(*ready)),
                Value::EnumVariant(_, variant, _) => Ok(Value::Bool(variant == "Ready")),
                _ => Ok(Value::Bool(false)),
            }
        });

        // is_pending: Check if a Poll value is Pending
        self.define_native("is_pending", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("is_pending: expected 1 argument"));
            }

            match &args[0] {
                Value::Poll(ready, _) => Ok(Value::Bool(!*ready)),
                Value::EnumVariant(_, variant, _) => Ok(Value::Bool(variant == "Pending")),
                _ => Ok(Value::Bool(false)),
            }
        });

        // unwrap_ready: Extract the value from Poll::Ready, panic if Pending
        self.define_native("unwrap_ready", |_env, args| {
            if args.len() != 1 {
                return Err(KainError::runtime("unwrap_ready: expected 1 argument"));
            }

            match &args[0] {
                Value::Poll(true, Some(val)) => Ok(*val.clone()),
                Value::Poll(true, None) => Ok(Value::Unit),
                Value::Poll(false, _) => {
                    Err(KainError::runtime("unwrap_ready: called on Poll::Pending"))
                }
                Value::EnumVariant(_, variant, fields) if variant == "Ready" => {
                    if fields.is_empty() {
                        Ok(Value::Unit)
                    } else {
                        Ok(fields[0].clone())
                    }
                }
                Value::EnumVariant(_, variant, _) if variant == "Pending" => {
                    Err(KainError::runtime("unwrap_ready: called on Poll::Pending"))
                }
                _ => Err(KainError::runtime("unwrap_ready: expected Poll value")),
            }
        });
    }

    fn define_native(&mut self, name: &str, func: fn(&mut Env, Vec<Value>) -> KainResult<Value>) {
        self.scopes[0].insert(name.to_string(), Value::NativeFn(name.to_string(), func));
    }

    fn define(&mut self, name: String, value: Value) {
        self.scopes.last_mut().unwrap().insert(name, value);
    }

    fn assign(&mut self, name: &str, value: Value) -> KainResult<()> {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), value);
                return Ok(());
            }
        }
        Err(KainError::runtime(format!("Undefined variable '{}'", name)))
    }

    fn lookup(&self, name: &str) -> Option<&Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v);
            }
        }
        None
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }
}

// === Evaluator ===

/// Interpret the program
pub fn interpret(program: &TypedProgram) -> KainResult<Value> {
    let mut env = Env::new();

    // Register functions
    for item in &program.items {
        match item {
            crate::types::TypedItem::Use(u) => {
                // Handle imports first
                load_module(&mut env, &u.ast)?;
            }
            crate::types::TypedItem::Function(f) => {
                env.functions.insert(f.ast.name.clone(), f.ast.clone());
                env.define(f.ast.name.clone(), Value::Function(f.ast.name.clone()));
            }
            crate::types::TypedItem::Actor(a) => {
                env.actor_defs.insert(a.ast.name.clone(), a.ast.clone());
            }
            crate::types::TypedItem::Component(c) => {
                env.components.insert(c.ast.name.clone(), c.ast.clone());
            }
            crate::types::TypedItem::Const(c) => {
                let val = eval_expr(&mut env, &c.ast.value)?;
                env.define(c.ast.name.clone(), val);
            }
            crate::types::TypedItem::Impl(i) => {
                // Get the type name
                let type_name = match &i.ast.target_type {
                    Type::Named { name, .. } => name.clone(),
                    _ => continue,
                };
                // Register all methods for this type
                let type_methods = env.methods.entry(type_name).or_insert_with(HashMap::new);
                for method in &i.ast.methods {
                    type_methods.insert(method.name.clone(), method.clone());
                }
            }
            crate::types::TypedItem::Comptime(_) => {} // Already evaluated
            _ => {}
        }
    }

    // Find and run main
    if let Some(main_fn) = env.functions.get("main").cloned() {
        eval_block(&mut env, &main_fn.body)
    } else {
        Ok(Value::Unit)
    }
}

fn load_module(env: &mut Env, u: &Use) -> KainResult<()> {
    let path = u.path.join("/");

    // Check if it's core stdlib (already loaded)
    if path == "stdlib" {
        return Ok(());
    }

    // Check for stdlib submodules: std/option, std/hashmap, std/result
    let file_path = if path.starts_with("std/") || path.starts_with("stdlib/") {
        // Look in the stdlib directory relative to the executable or crate root
        let module_name = path
            .trim_start_matches("std/")
            .trim_start_matches("stdlib/");

        // Try multiple locations for stdlib
        let possible_paths = [
            format!("stdlib/{}.kn", module_name),
            format!("../stdlib/{}.kn", module_name),
            format!("examples/../stdlib/{}.kn", module_name),
        ];

        possible_paths
            .into_iter()
            .map(|p| std::path::PathBuf::from(p))
            .find(|p| p.exists())
            .ok_or_else(|| {
                KainError::runtime(format!("Stdlib module not found: {}", module_name))
            })?
    } else {
        // Regular file path - try multiple locations
        let base_path = std::path::Path::new(&path);

        // Try various locations in order
        let possible_paths = [
            base_path.with_extension("kn"), // ./compiler/lexer.kn
            std::path::PathBuf::from(format!("src/{}.kn", path)), // src/compiler/lexer.kn
            std::path::PathBuf::from(format!("{}.kn", path)), // compiler/lexer.kn
            base_path.with_extension("god"), // legacy .god extension
        ];

        possible_paths
            .iter()
            .find(|p| p.exists())
            .cloned()
            .ok_or_else(|| {
                KainError::runtime(format!(
                    "Module not found: {} (tried: {:?})",
                    path,
                    possible_paths
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                ))
            })?
    };

    let source = std::fs::read_to_string(&file_path)
        .map_err(|e| KainError::runtime(format!("Failed to read module {}: {}", path, e)))?;

    let lexer = Lexer::new(&source);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(&tokens);
    let program = parser.parse()?;

    // Register items
    for item in program.items {
        match item {
            Item::Function(f) => {
                env.functions.insert(f.name.clone(), f.clone());
                env.define(f.name.clone(), Value::Function(f.name.clone()));
            }
            Item::Component(c) => {
                env.components.insert(c.name.clone(), c);
            }
            Item::Struct(s) => {
                let field_names = s.fields.iter().map(|f| f.name.clone()).collect();
                env.define(
                    s.name.clone(),
                    Value::StructConstructor(s.name.clone(), field_names),
                );
            }
            Item::Enum(e) => {
                // Register enum variants as constructors
                for variant in &e.variants {
                    let variant_name = format!("{}::{}", e.name, variant.name);
                    env.define(
                        variant_name,
                        Value::Function(format!("{}::{}", e.name, variant.name)),
                    );
                }
            }
            Item::Actor(a) => {
                env.actor_defs.insert(a.name.clone(), a);
            }
            Item::Const(c) => {
                let val = eval_expr(env, &c.value)?;
                env.define(c.name.clone(), val);
            }
            Item::Impl(i) => {
                if let Type::Named { name, .. } = &i.target_type {
                    // First, collect lowered function registrations
                    let lowered_fns: Vec<(String, Function)> = i
                        .methods
                        .iter()
                        .map(|m| (format!("{}_{}", name, m.name), m.clone()))
                        .collect();

                    // Register lowered functions
                    for (lowered_name, method) in lowered_fns {
                        env.functions.insert(lowered_name.clone(), method);
                        env.define(lowered_name.clone(), Value::Function(lowered_name));
                    }

                    // Then register methods
                    let type_methods = env.methods.entry(name.clone()).or_insert_with(HashMap::new);
                    for method in &i.methods {
                        type_methods.insert(method.name.clone(), method.clone());
                    }
                }
            }
            Item::Use(u) => {
                load_module(env, &u)?;
            }
            _ => {}
        }
    }

    Ok(())
}

pub fn eval_block(env: &mut Env, block: &Block) -> KainResult<Value> {
    for stmt in &block.stmts {
        let result = eval_stmt(env, stmt)?;
        // Propagate control flow up
        match &result {
            Value::Return(_) | Value::Break(_) | Value::Continue => return Ok(result),
            _ => {}
        }
    }
    Ok(Value::Unit)
}

fn eval_stmt(env: &mut Env, stmt: &Stmt) -> KainResult<Value> {
    match stmt {
        Stmt::Expr(expr) => {
            let val = eval_expr(env, expr)?;
            // Propagate control flow
            match &val {
                Value::Return(_) | Value::Break(_) | Value::Continue => return Ok(val),
                _ => {}
            }
            Ok(Value::Unit)
        }
        Stmt::Let { pattern, value, .. } => {
            let val = if let Some(expr) = value {
                eval_expr(env, expr)?
            } else {
                Value::None
            };
            if let Value::Return(_) = val {
                return Ok(val);
            }

            // Simple binding
            if let Pattern::Binding { name, .. } = pattern {
                env.define(name.clone(), val);
            }
            Ok(Value::Unit)
        }
        Stmt::Return(expr, _) => {
            let val = if let Some(e) = expr {
                eval_expr(env, e)?
            } else {
                Value::Unit
            };
            if let Value::Return(_) = val {
                return Ok(val);
            }
            Ok(Value::Return(Box::new(val)))
        }
        Stmt::For {
            binding,
            iter,
            body,
            ..
        } => {
            let iter_val = eval_expr(env, iter)?;
            if let Value::Return(_) = iter_val {
                return Ok(iter_val);
            }

            if let Value::Array(arr) = iter_val {
                let arr = arr.read().unwrap().clone();
                for val in arr.iter() {
                    env.push_scope();
                    if let Pattern::Binding { name, .. } = binding {
                        env.define(name.clone(), val.clone());
                    }
                    let res = eval_block(env, body)?;
                    env.pop_scope();

                    match res {
                        Value::Return(_) => return Ok(res),
                        Value::Break(_) => break,
                        Value::Continue => continue,
                        _ => {}
                    }
                }
            } else if let Value::String(s) = iter_val {
                for c in s.chars() {
                    env.push_scope();
                    if let Pattern::Binding { name, .. } = binding {
                        env.define(name.clone(), Value::String(c.to_string()));
                    }
                    let res = eval_block(env, body)?;
                    env.pop_scope();

                    match res {
                        Value::Return(_) => return Ok(res),
                        Value::Break(_) => break,
                        Value::Continue => continue,
                        _ => {}
                    }
                }
            }
            Ok(Value::Unit)
        }
        Stmt::While {
            condition, body, ..
        } => {
            loop {
                let cond = eval_expr(env, condition)?;
                if let Value::Return(_) = cond {
                    return Ok(cond);
                }
                if let Value::Bool(false) = cond {
                    break;
                }

                let res = eval_block(env, body)?;
                match res {
                    Value::Return(_) => return Ok(res),
                    Value::Break(_) => break,
                    Value::Continue => continue,
                    _ => {}
                }
            }
            Ok(Value::Unit)
        }
        Stmt::Loop { body, .. } => loop {
            let res = eval_block(env, body)?;
            match res {
                Value::Return(_) => return Ok(res),
                Value::Break(val) => {
                    return Ok(val.map(|v| *v).unwrap_or(Value::Unit));
                }
                Value::Continue => continue,
                _ => {}
            }
        },
        Stmt::Break(expr, _) => {
            let val = if let Some(e) = expr {
                Some(Box::new(eval_expr(env, e)?))
            } else {
                None
            };
            Ok(Value::Break(val))
        }
        Stmt::Continue(_) => Ok(Value::Continue),
        _ => Ok(Value::Unit),
    }
}

fn eval_assignment(env: &mut Env, target: &Expr, value: Value) -> KainResult<()> {
    match target {
        Expr::Ident(name, _) => env.assign(name, value),
        Expr::Field { object, field, .. } => {
            let obj_val = eval_expr(env, object)?;
            if let Value::Struct(_, fields) = obj_val {
                fields.write().unwrap().insert(field.clone(), value);
            } else if let Value::ActorRef(r) = obj_val {
                if let Some(self_id) = env.self_actor_id {
                    if self_id == r.id {
                        return env.assign(field, value);
                    }
                }
                return Err(KainError::runtime("Cannot assign to remote actor fields"));
            } else {
                return Err(KainError::runtime(
                    "Field assignment only supported on structs",
                ));
            }
            Ok(())
        }
        Expr::Index { object, index, .. } => {
            let obj_val = eval_expr(env, object)?;
            let idx_val = eval_expr(env, index)?;
            match (obj_val, idx_val) {
                (Value::Array(arr), Value::Int(i)) => {
                    let i = i as usize;
                    let mut arr = arr.write().unwrap();
                    if i < arr.len() {
                        arr[i] = value;
                    } else {
                        return Err(KainError::runtime("Index out of bounds"));
                    }
                }
                _ => {
                    return Err(KainError::runtime(
                        "Index assignment only supported on arrays with int index",
                    ))
                }
            }
            Ok(())
        }
        _ => Err(KainError::runtime("Invalid assignment target")),
    }
}

pub fn eval_expr(env: &mut Env, expr: &Expr) -> KainResult<Value> {
    match expr {
        Expr::MethodCall {
            receiver,
            method,
            args,
            span: _,
        } => {
            // Handle method call: obj.method(args)
            let obj_val = eval_expr(env, receiver)?;
            if let Value::Return(_) = obj_val {
                return Ok(obj_val);
            }

            // Evaluate arguments
            let mut arg_vals = Vec::new();
            for arg in args {
                let v = eval_expr(env, &arg.value)?;
                if let Value::Return(_) = v {
                    return Ok(v);
                }
                arg_vals.push(v);
            }

            match obj_val {
                // Struct methods: StructName_method(obj, args)
                Value::Struct(ref name, _) | Value::Future(ref name, _) => {
                    let func_name = format!("{}_{}", name, method);

                    if let Some(func) = env.functions.get(&func_name).cloned() {
                        // Call function with self as first argument
                        env.push_scope();
                        env.define("self".to_string(), obj_val);

                        // Bind other params
                        let param_iter = if func
                            .params
                            .first()
                            .map(|p| p.name == "self")
                            .unwrap_or(false)
                        {
                            func.params.iter().skip(1)
                        } else {
                            func.params.iter().skip(0)
                        };

                        if param_iter.len() != arg_vals.len() {
                            return Err(KainError::runtime(format!(
                                "Method {} arg mismatch",
                                func_name
                            )));
                        }

                        for (param, arg) in param_iter.zip(arg_vals.into_iter()) {
                            env.define(param.name.clone(), arg);
                        }

                        let result = eval_block(env, &func.body)?;
                        env.pop_scope();

                        match result {
                            Value::Return(v) => Ok(*v),
                            v => Ok(v),
                        }
                    } else {
                        Err(KainError::runtime(format!(
                            "Method {} not found for type {}",
                            method, name
                        )))
                    }
                }

                // Native Type Methods (e.g. Array.push, String.len)
                Value::Array(_) => {
                    // Map common array methods to native functions
                    match method.as_str() {
                        "push" => {
                            if arg_vals.len() != 1 {
                                return Err(KainError::runtime("push expects 1 argument"));
                            }
                            if let Value::Array(arr) = obj_val {
                                arr.write().unwrap().push(arg_vals[0].clone());
                                Ok(Value::Unit)
                            } else {
                                unreachable!()
                            }
                        }
                        "len" => {
                            if let Value::Array(arr) = obj_val {
                                Ok(Value::Int(arr.read().unwrap().len() as i64))
                            } else {
                                unreachable!()
                            }
                        }
                        _ => Err(KainError::runtime(format!(
                            "Method {} not found on Array",
                            method
                        ))),
                    }
                }

                _ => Err(KainError::runtime(format!(
                    "Method calls not supported on this type: {:?}",
                    obj_val
                ))),
            }
        }

        Expr::Call { callee, args, .. } => {
            // Special case: Handle Type.method() or obj.method() calls
            if let Expr::Field { object, field, .. } = callee.as_ref() {
                // Check if this is a type-level static method call like RNG.new()
                if let Expr::Ident(type_name, _) = object.as_ref() {
                    // Check if it's a type with methods - clone to avoid borrow issues
                    let method = env
                        .methods
                        .get(type_name)
                        .and_then(|m| m.get(field))
                        .cloned();

                    if let Some(method) = method {
                        // Evaluate arguments
                        let mut arg_vals = Vec::new();
                        for arg in args {
                            let v = eval_expr(env, &arg.value)?;
                            if let Value::Return(_) = v {
                                return Ok(v);
                            }
                            arg_vals.push(v);
                        }

                        // Call the static method
                        env.push_scope();
                        for (param, arg) in method.params.iter().zip(arg_vals.into_iter()) {
                            env.define(param.name.clone(), arg);
                        }
                        let result = eval_block(env, &method.body);
                        env.pop_scope();

                        return match result? {
                            Value::Return(v) => Ok(*v),
                            v => Ok(v),
                        };
                    }
                }

                // Check if this is an instance method call like obj.method()
                let obj_val = eval_expr(env, object)?;
                if let Value::Return(_) = obj_val {
                    return Ok(obj_val);
                }

                // Get the type name from the value
                let type_name = match &obj_val {
                    Value::Struct(name, _) => Some(name.clone()),
                    _ => None,
                };

                if let Some(type_name) = type_name {
                    // Clone method to avoid borrow issues
                    let method = env
                        .methods
                        .get(&type_name)
                        .and_then(|m| m.get(field))
                        .cloned();

                    if let Some(method) = method {
                        // Evaluate arguments
                        let mut arg_vals = Vec::new();
                        for arg in args {
                            let v = eval_expr(env, &arg.value)?;
                            if let Value::Return(_) = v {
                                return Ok(v);
                            }
                            arg_vals.push(v);
                        }

                        // Call the instance method with `self` bound
                        env.push_scope();
                        env.define("self".to_string(), obj_val);

                        // Skip 'self' parameter if present in method definition
                        let params_iter = if let Some(first) = method.params.first() {
                            if first.name == "self" {
                                method.params.iter().skip(1)
                            } else {
                                method.params.iter().skip(0)
                            }
                        } else {
                            method.params.iter().skip(0)
                        };

                        for (param, arg) in params_iter.zip(arg_vals.into_iter()) {
                            env.define(param.name.clone(), arg);
                        }
                        let result = eval_block(env, &method.body);
                        env.pop_scope();

                        return match result? {
                            Value::Return(v) => Ok(*v),
                            v => Ok(v),
                        };
                    }
                }
            }

            // Normal function call
            let func_val = {
                let v = eval_expr(env, callee)?;
                if let Value::Return(_) = v {
                    return Ok(v);
                }
                v
            };

            // Evaluate arguments
            let mut arg_vals = Vec::new();
            for arg in args {
                let v = eval_expr(env, &arg.value)?;
                if let Value::Return(_) = v {
                    return Ok(v);
                }
                arg_vals.push(v);
            }

            call_function(env, func_val, arg_vals)
        }

        Expr::Try(inner, _) => {
            let val = eval_expr(env, inner)?;
            if let Value::Return(_) = val {
                return Ok(val);
            }
            match val {
                Value::Result(true, v) => Ok(*v),
                Value::Result(false, e) => Ok(Value::Return(Box::new(Value::Result(false, e)))),
                _ => Err(KainError::runtime(
                    "Type error: expected Result for ? operator",
                )),
            }
        }

        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            let cond = eval_expr(env, condition)?;
            if let Value::Return(_) = cond {
                return Ok(cond);
            }
            if let Value::Bool(true) = cond {
                eval_block(env, then_branch)
            } else if let Some(eb) = else_branch {
                match eb.as_ref() {
                    ElseBranch::Else(block) => eval_block(env, block),
                    _ => Ok(Value::Unit),
                }
            } else {
                Ok(Value::Unit)
            }
        }

        Expr::Match {
            scrutinee, arms, ..
        } => {
            let val = eval_expr(env, scrutinee)?;
            if let Value::Return(_) = val {
                return Ok(val);
            }

            for arm in arms {
                if pattern_matches(&arm.pattern, &val) {
                    env.push_scope();
                    bind_pattern(env, &arm.pattern, &val);
                    let res = eval_expr(env, &arm.body)?;
                    env.pop_scope();
                    return Ok(res);
                }
            }
            // If no match, check if it's exhaustive or return unit?
            Ok(Value::Unit)
        }

        Expr::MacroCall { name, args, .. } => {
            // Built-in macros
            match name.as_str() {
                "vec" => {
                    let mut vals = Vec::new();
                    for arg in args {
                        let v = eval_expr(env, arg)?;
                        if let Value::Return(_) = v {
                            return Ok(v);
                        }
                        vals.push(v);
                    }
                    Ok(Value::Array(Arc::new(RwLock::new(vals))))
                }
                "format" => {
                    // TODO: proper format
                    let mut res = String::new();
                    for arg in args {
                        let v = eval_expr(env, arg)?;
                        res.push_str(&format!("{}", v));
                    }
                    Ok(Value::String(res))
                }
                "type_name" => {
                    if let Some(arg) = args.first() {
                        let v = eval_expr(env, arg)?;
                        let type_name = match v {
                            Value::Unit => "unit",
                            Value::Bool(_) => "bool",
                            Value::Int(_) => "int",
                            Value::Float(_) => "float",
                            Value::String(_) => "string",
                            Value::Array(_) => "array",
                            Value::Tuple(_) => "tuple",
                            Value::Struct(name, _) => return Ok(Value::String(name.clone())),
                            Value::Function(_) => "function",
                            Value::NativeFn(_, _) => "native_fn",
                            Value::StructConstructor(_, _) => "struct_constructor",
                            Value::ActorRef(_) => "actor",
                            Value::None => "none",
                            Value::Return(_) => "return",
                            Value::Result(_, _) => "result",
                            Value::Closure(_, _, _) => "closure",
                            Value::JSX(_) => "jsx",
                            Value::EnumVariant(enum_name, _, _) => {
                                return Ok(Value::String(enum_name.clone()))
                            }
                            Value::Poll(_, _) => "poll",
                            Value::Future(name, _) => {
                                return Ok(Value::String(format!("Future<{}>", name)))
                            }
                            Value::Break(_) => "break",
                            Value::Continue => "continue",
                        };
                        Ok(Value::String(type_name.to_string()))
                    } else {
                        Err(KainError::runtime("type_name! requires an argument"))
                    }
                }
                _ => Err(KainError::runtime(format!("Unknown macro: {}!", name))),
            }
        }
        Expr::Assign { target, value, .. } => {
            let v = eval_expr(env, value)?;
            if let Value::Return(_) = v {
                return Ok(v);
            }
            eval_assignment(env, target, v)?;
            Ok(Value::Unit)
        }
        Expr::Int(n, _) => Ok(Value::Int(*n)),
        Expr::Float(n, _) => Ok(Value::Float(*n)),
        Expr::String(s, _) => Ok(Value::String(s.clone())),
        Expr::FString(parts, _) => {
            let mut result = String::new();
            for part in parts {
                let val = eval_expr(env, part)?;
                if let Value::Return(_) = val {
                    return Ok(val);
                }
                result.push_str(&format!("{}", val));
            }
            Ok(Value::String(result))
        }
        Expr::Bool(b, _) => Ok(Value::Bool(*b)),
        Expr::None(_) => Ok(Value::None),
        Expr::Lambda { params, body, .. } => {
            let param_names = params.iter().map(|p| p.name.clone()).collect();
            Ok(Value::Closure(
                param_names,
                body.clone(),
                env.scopes.clone(),
            ))
        }
        Expr::Ident(name, _span) => env
            .lookup(name)
            .cloned()
            .ok_or_else(|| KainError::runtime(format!("Undefined: {}", name))),

        Expr::Binary {
            left, op, right, ..
        } => {
            let l = eval_expr(env, left)?;
            if let Value::Return(_) = l {
                return Ok(l);
            }
            let r = eval_expr(env, right)?;
            if let Value::Return(_) = r {
                return Ok(r);
            }
            eval_binop(*op, l, r)
        }

        Expr::Unary { op, operand, .. } => {
            let v = eval_expr(env, operand)?;
            if let Value::Return(_) = v {
                return Ok(v);
            }
            match (op, v) {
                (UnaryOp::Neg, Value::Int(n)) => Ok(Value::Int(-n)),
                (UnaryOp::Neg, Value::Float(n)) => Ok(Value::Float(-n)),
                (UnaryOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
                _ => Err(KainError::runtime("Invalid unary operation")),
            }
        }

        Expr::Array(elements, _) => {
            let mut vals = Vec::new();
            for elem in elements {
                let v = eval_expr(env, elem)?;
                if let Value::Return(_) = v {
                    return Ok(v);
                }
                vals.push(v);
            }
            Ok(Value::Array(Arc::new(RwLock::new(vals))))
        }

        Expr::Index { object, index, .. } => {
            let obj = eval_expr(env, object)?;
            if let Value::Return(_) = obj {
                return Ok(obj);
            }
            let idx = eval_expr(env, index)?;
            if let Value::Return(_) = idx {
                return Ok(idx);
            }

            match (obj, idx) {
                (Value::Array(arr), Value::Int(i)) => {
                    let i = i as usize;
                    let arr = arr.read().unwrap();
                    if i < arr.len() {
                        Ok(arr[i].clone())
                    } else {
                        Err(KainError::runtime(format!("Index out of bounds: {}", i)))
                    }
                }
                (Value::String(s), Value::Int(i)) => {
                    let i = i as usize;
                    if i < s.len() {
                        Ok(Value::String(s.chars().nth(i).unwrap().to_string()))
                    } else {
                        Err(KainError::runtime(format!("Index out of bounds: {}", i)))
                    }
                }
                _ => Err(KainError::runtime(
                    "Index operator requires array/string and int",
                )),
            }
        }

        // Structure creation
        Expr::Struct { name, fields, .. } => {
            let mut field_vals = HashMap::new();
            for (k, expr) in fields {
                let v = eval_expr(env, expr)?;
                if let Value::Return(_) = v {
                    return Ok(v);
                }
                field_vals.insert(k.clone(), v);
            }
            Ok(Value::Struct(
                name.clone(),
                Arc::new(RwLock::new(field_vals)),
            ))
        }

        // JSX
        Expr::JSX(node, _) => eval_jsx(env, node),

        Expr::Field { object, field, .. } => {
            let obj_val = eval_expr(env, object)?;
            if let Value::Return(_) = obj_val {
                return Ok(obj_val);
            }

            match obj_val {
                Value::Struct(_, fields) => {
                    let fields = fields.read().unwrap();
                    fields
                        .get(field)
                        .cloned()
                        .ok_or_else(|| KainError::runtime(format!("Field not found: {}", field)))
                }
                Value::ActorRef(r) => {
                    // Check if it's the current actor (self)
                    if let Some(self_id) = env.self_actor_id {
                        if self_id == r.id {
                            return env.lookup(field).cloned().ok_or_else(|| {
                                KainError::runtime(format!("Actor field not found: {}", field))
                            });
                        }
                    }

                    // Allow accessing actor fields? For now maybe just id
                    if field == "id" {
                        return Ok(Value::Int(r.id as i64));
                    }
                    Err(KainError::runtime("Actor fields not accessible"))
                }
                _ => Err(KainError::runtime(format!(
                    "Field access on non-struct value: {:?}",
                    obj_val
                ))),
            }
        }

        Expr::Tuple(elements, _) => {
            let mut vals = Vec::new();
            for elem in elements {
                let v = eval_expr(env, elem)?;
                if let Value::Return(_) = v {
                    return Ok(v);
                }
                vals.push(v);
            }
            Ok(Value::Tuple(vals))
        }

        Expr::Spawn { actor, init, .. } => {
            // Find actor definition
            let actor_def = env
                .actor_defs
                .get(actor)
                .cloned()
                .ok_or_else(|| KainError::runtime(format!("Unknown actor: {}", actor)))?;

            // Evaluate init expressions
            let mut init_vals = HashMap::new();
            for (field, expr) in init {
                let v = eval_expr(env, expr)?;
                if let Value::Return(_) = v {
                    return Ok(v);
                }
                init_vals.insert(field.clone(), v);
            }

            // Create channel
            let (tx, rx) = flume::unbounded();
            let id = env.next_actor_id;
            env.next_actor_id += 1;
            let sender = tx.clone();
            env.actors.insert(id, sender.clone());

            // Spawn thread
            let functions = env.functions.clone();
            let components = env.components.clone();
            let actor_defs = env.actor_defs.clone();
            let methods = env.methods.clone();
            let global_scope = env.scopes.first().cloned().unwrap_or_default();
            let actor_name = actor.clone();
            let self_sender = tx.clone();

            std::thread::spawn(move || {
                let mut actor_env = Env {
                    scopes: vec![global_scope],
                    functions,
                    components,
                    methods,
                    actors: HashMap::new(),
                    next_actor_id: 0,
                    actor_defs,
                    self_actor_id: Some(id),
                    python_scope: None,
                };

                // Initialize Python scope
                Python::with_gil(|py| {
                    let locals = PyDict::new(py);
                    actor_env.python_scope = Some(locals.into());
                });

                actor_env.register_stdlib();

                actor_env.push_scope(); // Actor scope

                // Define self
                let actor_val = Value::ActorRef(ActorRef {
                    id,
                    sender: self_sender,
                });
                actor_env.define("self".to_string(), actor_val);

                // Initialize state
                for state_decl in &actor_def.state {
                    if let Some(val) = init_vals.get(&state_decl.name) {
                        actor_env.define(state_decl.name.clone(), val.clone());
                    } else {
                        // Evaluate default value
                        match eval_expr(&mut actor_env, &state_decl.initial) {
                            Ok(val) => actor_env.define(state_decl.name.clone(), val),
                            Err(e) => {
                                eprintln!("Actor initialization error: {}", e);
                                return;
                            }
                        }
                    }
                }

                // Event loop
                while let Ok(msg) = rx.recv() {
                    // Find handler
                    let mut handled = false;
                    for handler in &actor_def.handlers {
                        if handler.message_type == msg.name {
                            // Run handler
                            actor_env.push_scope();
                            // Bind params by position
                            for (i, param) in handler.params.iter().enumerate() {
                                if let Some(val) = msg.args.get(i) {
                                    actor_env.define(param.name.clone(), val.clone());
                                }
                            }

                            if let Err(e) = eval_block(&mut actor_env, &handler.body) {
                                println!("Error in actor handler {}: {}", handler.message_type, e);
                            }
                            actor_env.pop_scope();
                            handled = true;
                            break;
                        }
                    }
                    if !handled {
                        println!(
                            "Actor {} received unknown message: {}",
                            actor_name, msg.name
                        );
                    }
                }
            });

            Ok(Value::ActorRef(ActorRef { id, sender }))
        }

        Expr::SendMsg {
            target,
            message,
            data,
            ..
        } => {
            let actor_val = eval_expr(env, target)?;
            if let Value::Return(_) = actor_val {
                return Ok(actor_val);
            }

            if let Value::ActorRef(r) = actor_val {
                let mut msg_args = Vec::new();
                for (_name, expr) in data {
                    let v = eval_expr(env, expr)?;
                    msg_args.push(v);
                }

                let msg = Message {
                    name: message.clone(),
                    args: msg_args,
                };

                let _ = r.sender.send(msg);
                Ok(Value::Unit)
            } else {
                Err(KainError::runtime("send target must be an actor"))
            }
        }

        // Block expression: { stmts }
        Expr::Block(block, _) => eval_block(env, block),

        // Return expression in expression context
        Expr::Return(expr, _) => {
            let val = if let Some(e) = expr {
                eval_expr(env, e)?
            } else {
                Value::Unit
            };
            Ok(Value::Return(Box::new(val)))
        }

        Expr::Paren(inner, _) => eval_expr(env, inner),

        // Await expression: await future_expr
        // Uses the async runtime to poll the future to completion
        Expr::Await(future_expr, _span) => {
            let future_val = eval_expr(env, future_expr)?;
            if let Value::Return(_) = future_val {
                return Ok(future_val);
            }

            // Use the async runtime to poll to completion
            poll_future_to_completion(env, future_val)
        }

        // OR static method call: TypeName::method(args)
        Expr::EnumVariant {
            enum_name,
            variant,
            fields,
            ..
        } => {
            // First, check if this is a static method call
            // Check if enum_name is a type with methods and variant is a method name
            if let Some(type_methods) = env.methods.get(enum_name).cloned() {
                if let Some(method) = type_methods.get(variant).cloned() {
                    // This is a static method call like Lexer::new(source)
                    let arg_vals: Vec<Value> = match fields {
                        EnumVariantFields::Unit => Vec::new(),
                        EnumVariantFields::Tuple(exprs) => {
                            let mut vals = Vec::new();
                            for e in exprs {
                                let v = eval_expr(env, e)?;
                                if let Value::Return(_) = v {
                                    return Ok(v);
                                }
                                vals.push(v);
                            }
                            vals
                        }
                        EnumVariantFields::Struct(named_fields) => {
                            let mut vals = Vec::new();
                            for (_, e) in named_fields {
                                let v = eval_expr(env, e)?;
                                if let Value::Return(_) = v {
                                    return Ok(v);
                                }
                                vals.push(v);
                            }
                            vals
                        }
                    };

                    // Call the static method
                    env.push_scope();
                    for (param, arg) in method.params.iter().zip(arg_vals.into_iter()) {
                        env.define(param.name.clone(), arg);
                    }
                    let result = eval_block(env, &method.body)?;
                    env.pop_scope();

                    return match result {
                        Value::Return(v) => Ok(*v),
                        v => Ok(v),
                    };
                }
            }

            // Check for lowered function name: Type_method (from monomorphization)
            let lowered_name = format!("{}_{}", enum_name, variant);
            if let Some(func) = env.functions.get(&lowered_name).cloned() {
                // This is a lowered method call (Type_method from monomorphization)
                let arg_vals: Vec<Value> = match fields {
                    EnumVariantFields::Unit => Vec::new(),
                    EnumVariantFields::Tuple(exprs) => {
                        let mut vals = Vec::new();
                        for e in exprs {
                            let v = eval_expr(env, e)?;
                            if let Value::Return(_) = v {
                                return Ok(v);
                            }
                            vals.push(v);
                        }
                        vals
                    }
                    EnumVariantFields::Struct(named_fields) => {
                        let mut vals = Vec::new();
                        for (_, e) in named_fields {
                            let v = eval_expr(env, e)?;
                            if let Value::Return(_) = v {
                                return Ok(v);
                            }
                            vals.push(v);
                        }
                        vals
                    }
                };

                // Call the lowered function
                env.push_scope();
                for (param, arg) in func.params.iter().zip(arg_vals.into_iter()) {
                    env.define(param.name.clone(), arg);
                }
                let result = eval_block(env, &func.body)?;
                env.pop_scope();

                return match result {
                    Value::Return(v) => Ok(*v),
                    v => Ok(v),
                };
            }

            // Not a static method call - proceed with enum variant construction
            let field_vals = match fields {
                EnumVariantFields::Unit => Vec::new(),
                EnumVariantFields::Tuple(exprs) => {
                    let mut vals = Vec::new();
                    for e in exprs {
                        let v = eval_expr(env, e)?;
                        if let Value::Return(_) = v {
                            return Ok(v);
                        }
                        vals.push(v);
                    }
                    vals
                }
                EnumVariantFields::Struct(named_fields) => {
                    let mut vals = Vec::new();
                    for (_, e) in named_fields {
                        let v = eval_expr(env, e)?;
                        if let Value::Return(_) = v {
                            return Ok(v);
                        }
                        vals.push(v);
                    }
                    vals
                }
            };

            // Special case: Poll enum gets native representation
            if enum_name == "Poll" {
                match variant.as_str() {
                    "Ready" => {
                        let inner = if field_vals.is_empty() {
                            None
                        } else {
                            Some(Box::new(
                                field_vals.into_iter().next().unwrap_or(Value::Unit),
                            ))
                        };
                        Ok(Value::Poll(true, inner))
                    }
                    "Pending" => Ok(Value::Poll(false, None)),
                    _ => Ok(Value::EnumVariant(
                        enum_name.clone(),
                        variant.clone(),
                        field_vals,
                    )),
                }
            } else {
                Ok(Value::EnumVariant(
                    enum_name.clone(),
                    variant.clone(),
                    field_vals,
                ))
            }
        }

        Expr::Break(expr, _) => {
            let val = if let Some(e) = expr {
                Some(Box::new(eval_expr(env, e)?))
            } else {
                None
            };
            Ok(Value::Break(val))
        }

        Expr::Continue(_) => Ok(Value::Continue),

        _ => Err(KainError::runtime(format!(
            "Expression not supported in runtime: {:?}",
            expr
        ))),
    }
}

fn call_function(env: &mut Env, func: Value, args: Vec<Value>) -> KainResult<Value> {
    match func {
        Value::Function(name) => {
            let f = env
                .functions
                .get(&name)
                .cloned()
                .ok_or_else(|| KainError::runtime(format!("Function not found: {}", name)))?;
            if f.params.len() != args.len() {
                return Err(KainError::runtime(format!(
                    "Argument mismatch: expected {}, got {}",
                    f.params.len(),
                    args.len()
                )));
            }

            env.push_scope();
            for (param, arg) in f.params.iter().zip(args.into_iter()) {
                env.define(param.name.clone(), arg);
            }

            let result = eval_block(env, &f.body)?;
            env.pop_scope();

            match result {
                Value::Return(v) => Ok(*v),
                v => Ok(v),
            }
        }
        Value::NativeFn(_, f) => f(env, args),
        Value::Closure(params, body, captured) => {
            if params.len() != args.len() {
                return Err(KainError::runtime(format!("Closure arg mismatch")));
            }

            // Restore captured scope + new scope
            let old_scopes = env.scopes.clone();
            env.scopes = captured;
            env.push_scope();

            for (name, arg) in params.iter().zip(args.into_iter()) {
                env.define(name.clone(), arg);
            }

            let result = eval_expr(env, &body)?;

            env.pop_scope();
            env.scopes = old_scopes;

            match result {
                Value::Return(v) => Ok(*v),
                v => Ok(v),
            }
        }
        Value::StructConstructor(name, fields) => {
            if fields.len() != args.len() {
                return Err(KainError::runtime(format!(
                    "Struct constructor for {} expected {} arguments, got {}",
                    name,
                    fields.len(),
                    args.len()
                )));
            }

            let mut field_vals = HashMap::new();
            for (i, val) in args.into_iter().enumerate() {
                field_vals.insert(fields[i].clone(), val);
            }

            Ok(Value::Struct(name, Arc::new(RwLock::new(field_vals))))
        }
        _ => Err(KainError::runtime("Not a function")),
    }
}

fn eval_binop(op: BinaryOp, left: Value, right: Value) -> KainResult<Value> {
    match (op, &left, &right) {
        (BinaryOp::Add, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
        (BinaryOp::Sub, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
        (BinaryOp::Mul, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
        (BinaryOp::Div, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a / b)),
        (BinaryOp::Mod, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a % b)),
        (BinaryOp::Add, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
        (BinaryOp::Sub, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
        (BinaryOp::Mul, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
        (BinaryOp::Div, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
        (BinaryOp::Add, Value::String(a), Value::String(b)) => Ok(Value::String(a.to_owned() + b)),
        (BinaryOp::Eq, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a == b)),
        (BinaryOp::Ne, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a != b)),
        (BinaryOp::Eq, Value::String(a), Value::String(b)) => Ok(Value::Bool(a == b)),
        (BinaryOp::Ne, Value::String(a), Value::String(b)) => Ok(Value::Bool(a != b)),
        (BinaryOp::Eq, Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a == b)),
        (BinaryOp::Ne, Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a != b)),

        // Float comparisons
        (BinaryOp::Lt, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a < b)),
        (BinaryOp::Gt, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a > b)),
        (BinaryOp::Le, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a <= b)),
        (BinaryOp::Ge, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a >= b)),
        (BinaryOp::Eq, Value::Float(a), Value::Float(b)) => {
            Ok(Value::Bool((a - b).abs() < f64::EPSILON))
        }
        (BinaryOp::Ne, Value::Float(a), Value::Float(b)) => {
            Ok(Value::Bool((a - b).abs() >= f64::EPSILON))
        }

        (BinaryOp::Lt, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a < b)),
        (BinaryOp::Gt, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a > b)),
        (BinaryOp::Le, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a <= b)),
        (BinaryOp::Ge, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a >= b)),
        (BinaryOp::And, Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(*a && *b)),
        (BinaryOp::Or, Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(*a || *b)),
        (BinaryOp::Eq, Value::None, Value::None) => Ok(Value::Bool(true)),
        (BinaryOp::Ne, Value::None, Value::None) => Ok(Value::Bool(false)),
        (BinaryOp::Eq, Value::Unit, Value::Unit) => Ok(Value::Bool(true)),
        (BinaryOp::Ne, Value::Unit, Value::Unit) => Ok(Value::Bool(false)),
        (BinaryOp::Eq, _, _) => Ok(Value::Bool(false)),
        (BinaryOp::Ne, _, _) => Ok(Value::Bool(true)),

        // Error on mismatch unless one is Any?
        _ => Err(KainError::runtime(format!(
            "Type mismatch in binary operation: {:?} {:?} {:?}",
            left, op, right
        ))),
    }
}

fn pattern_matches(pattern: &Pattern, value: &Value) -> bool {
    match pattern {
        Pattern::Wildcard(_) => true,
        Pattern::Binding { .. } => true,
        Pattern::Literal(Expr::Int(n, _)) => matches!(value, Value::Int(v) if *v == *n),
        Pattern::Literal(Expr::String(s, _)) => matches!(value, Value::String(v) if v == s),
        Pattern::Literal(Expr::Bool(b, _)) => matches!(value, Value::Bool(v) if *v == *b),
        Pattern::Variant {
            variant, fields, ..
        } => {
            if let Value::Poll(ready, val) = value {
                if *variant == "Ready" {
                    if !ready {
                        return false;
                    }
                    if let VariantPatternFields::Tuple(pats) = fields {
                        if pats.len() == 1 {
                            return if let Some(v) = val {
                                pattern_matches(&pats[0], v)
                            } else {
                                false
                            };
                        }
                    }
                    return false;
                } else if *variant == "Pending" {
                    return !ready;
                }
                return false;
            }
            if let Value::EnumVariant(_, v_name, v_fields) = value {
                if variant != v_name {
                    return false;
                }
                match fields {
                    VariantPatternFields::Unit => v_fields.is_empty(),
                    VariantPatternFields::Tuple(pats) => {
                        if pats.len() != v_fields.len() {
                            return false;
                        }
                        pats.iter()
                            .zip(v_fields.iter())
                            .all(|(p, v)| pattern_matches(p, v))
                    }
                    _ => false,
                }
            } else {
                false
            }
        }
        _ => false,
    }
}

fn bind_pattern(env: &mut Env, pattern: &Pattern, value: &Value) {
    match pattern {
        Pattern::Binding { name, .. } => {
            env.define(name.clone(), value.clone());
        }
        Pattern::Variant {
            variant, fields, ..
        } => {
            if let Value::Poll(ready, val) = value {
                if *variant == "Ready" && *ready {
                    if let VariantPatternFields::Tuple(pats) = fields {
                        if pats.len() == 1 {
                            if let Some(v) = val {
                                bind_pattern(env, &pats[0], v);
                            }
                        }
                    }
                }
            } else if let Value::EnumVariant(_, _, v_fields) = value {
                match fields {
                    VariantPatternFields::Tuple(pats) => {
                        for (p, v) in pats.iter().zip(v_fields.iter()) {
                            bind_pattern(env, p, v);
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn eval_jsx(env: &mut Env, node: &JSXNode) -> KainResult<Value> {
    match node {
        JSXNode::Element {
            tag,
            attributes,
            children,
            ..
        } => {
            let mut attr_vals = HashMap::new();
            for attr in attributes {
                let v = match &attr.value {
                    JSXAttrValue::String(s) => Value::String(s.clone()),
                    JSXAttrValue::Bool(b) => Value::Bool(*b),
                    JSXAttrValue::Expr(e) => eval_expr(env, e)?,
                };
                if let Value::Return(_) = v {
                    return Ok(v);
                }
                attr_vals.insert(attr.name.clone(), v);
            }

            let mut child_vals = Vec::new();
            for child in children {
                let v = eval_jsx(env, child)?;
                if let Value::Return(_) = v {
                    return Ok(v);
                }
                match v {
                    Value::JSX(node) => child_vals.push(node),
                    Value::String(s) => child_vals.push(VNode::Text(s)),
                    Value::Int(n) => child_vals.push(VNode::Text(n.to_string())),
                    Value::Float(n) => child_vals.push(VNode::Text(n.to_string())),
                    _ => {}
                }
            }

            Ok(Value::JSX(VNode::Element {
                tag: tag.clone(),
                attrs: attr_vals,
                children: child_vals,
            }))
        }
        JSXNode::Text(s, _) => Ok(Value::String(s.clone())),
        JSXNode::Expression(expr) => eval_expr(env, expr),
        _ => Ok(Value::Unit),
    }
}

/// Run all tests in the program
pub fn run_tests(program: &TypedProgram) -> KainResult<()> {
    println!("\n Running Tests...\n");
    let mut passed = 0;
    let mut failed = 0;

    // Initialize env
    let mut env = Env::new();

    // Register items first (functions, etc.)
    for item in &program.items {
        match item {
            crate::types::TypedItem::Function(f) => {
                env.functions.insert(f.ast.name.clone(), f.ast.clone());
                // Also define in scope for lookup
                env.define(f.ast.name.clone(), Value::Function(f.ast.name.clone()));
            }
            crate::types::TypedItem::Actor(a) => {
                env.actor_defs.insert(a.ast.name.clone(), a.ast.clone());
            }
            crate::types::TypedItem::Component(c) => {
                env.components.insert(c.ast.name.clone(), c.ast.clone());
            }
            crate::types::TypedItem::Const(c) => {
                let val = eval_expr(&mut env, &c.ast.value)?;
                env.define(c.ast.name.clone(), val);
            }
            crate::types::TypedItem::Impl(i) => {
                let type_name = match &i.ast.target_type {
                    Type::Named { name, .. } => name.clone(),
                    _ => continue,
                };
                let type_methods = env.methods.entry(type_name).or_insert_with(HashMap::new);
                for method in &i.ast.methods {
                    type_methods.insert(method.name.clone(), method.clone());
                }
            }
            crate::types::TypedItem::Use(u) => {
                load_module(&mut env, &u.ast)?;
            }
            _ => {}
        }
    }

    // Run tests
    for item in &program.items {
        if let crate::types::TypedItem::Test(test) = item {
            print!("test {} ... ", test.ast.name);

            // Isolate test scope
            env.push_scope();

            match eval_block(&mut env, &test.ast.body) {
                Ok(_) => {
                    println!("ok");
                    passed += 1;
                }
                Err(e) => {
                    println!("FAILED");
                    println!("  Error: {}", e);
                    failed += 1;
                }
            }

            env.pop_scope();
        }
    }

    println!(
        "\nTest result: {}. {} passed; {} failed",
        if failed == 0 { "ok" } else { "FAILED" },
        passed,
        failed
    );

    if failed > 0 {
        Err(KainError::runtime("Some tests failed"))
    } else {
        Ok(())
    }
}

// === ASYNC RUNTIME HELPERS ===

/// Poll a future repeatedly until it returns Ready
fn poll_future_to_completion(env: &mut Env, future_val: Value) -> KainResult<Value> {
    let max_iterations = 100000; // Prevent infinite loops
    let mut iterations = 0;
    let current_future = future_val;

    loop {
        iterations += 1;
        if iterations > max_iterations {
            return Err(KainError::runtime("Async timeout: future did not complete"));
        }

        let poll_result = poll_future_once(env, current_future.clone())?;

        match extract_poll_result(&poll_result) {
            PollState::Ready(val) => return Ok(val),
            PollState::Pending => {
                // In a real async runtime, we'd yield to other tasks here
                // For now, just continue polling (cooperative busy-wait)
                std::thread::sleep(std::time::Duration::from_micros(10));
                continue;
            }
            PollState::NotAPoll => {
                // Not a recognizable poll result - return as-is (might be already resolved)
                return Ok(poll_result);
            }
        }
    }
}

/// Poll a future exactly once and return the Poll result
fn poll_future_once(env: &mut Env, future_val: Value) -> KainResult<Value> {
    match &future_val {
        // Handle Future struct (from async fn transformation)
        Value::Future(struct_name, state) => {
            let poll_fn_name = format!("{}_poll", struct_name);

            if let Some(poll_fn) = env.functions.get(&poll_fn_name).cloned() {
                // Create a temporary struct value from the state
                let struct_val = Value::Struct(struct_name.clone(), state.clone());

                // Call poll function with self parameter
                env.push_scope();
                env.define("self".to_string(), struct_val.clone());

                if let Some(first_param) = poll_fn.params.first() {
                    env.define(first_param.name.clone(), struct_val);
                }

                let result = eval_block(env, &poll_fn.body)?;
                env.pop_scope();

                // Unwrap Value::Return if present
                let actual_result = match result {
                    Value::Return(v) => *v,
                    v => v,
                };

                // Normalize the result to our Poll representation
                Ok(normalize_poll_result(actual_result))
            } else {
                // No poll function - treat as immediately ready with unit
                Ok(Value::Poll(true, Some(Box::new(Value::Unit))))
            }
        }

        // Handle plain struct that might be a future
        Value::Struct(struct_name, _) => {
            let poll_fn_name = format!("{}_poll", struct_name);

            if let Some(poll_fn) = env.functions.get(&poll_fn_name).cloned() {
                // Call poll with the future as self
                env.push_scope();
                env.define("self".to_string(), future_val.clone());

                if let Some(first_param) = poll_fn.params.first() {
                    env.define(first_param.name.clone(), future_val.clone());
                }

                let result = eval_block(env, &poll_fn.body)?;
                env.pop_scope();

                // Unwrap Value::Return if present
                let actual_result = match result {
                    Value::Return(v) => *v,
                    v => v,
                };

                Ok(normalize_poll_result(actual_result))
            } else {
                // No poll function - might be an already-resolved value
                Ok(Value::Poll(true, Some(Box::new(future_val))))
            }
        }

        // Already a Poll value - return as-is
        Value::Poll(_, _) => Ok(future_val),

        // EnumVariant that might be Poll::Ready or Poll::Pending
        Value::EnumVariant(enum_name, _, _) if enum_name == "Poll" => {
            Ok(normalize_poll_result(future_val))
        }

        // Non-future value - treat as immediately ready
        _ => Ok(Value::Poll(true, Some(Box::new(future_val)))),
    }
}

/// Internal enum for poll state extraction
enum PollState {
    Ready(Value),
    Pending,
    NotAPoll,
}

/// Extract the poll state from a value
fn extract_poll_result(val: &Value) -> PollState {
    match val {
        // Native Poll value
        Value::Poll(true, Some(inner)) => PollState::Ready(*inner.clone()),
        Value::Poll(true, None) => PollState::Ready(Value::Unit),
        Value::Poll(false, _) => PollState::Pending,

        // EnumVariant style Poll::Ready/Poll::Pending
        Value::EnumVariant(enum_name, variant, fields) if enum_name == "Poll" => {
            match variant.as_str() {
                "Ready" => {
                    if fields.is_empty() {
                        PollState::Ready(Value::Unit)
                    } else {
                        PollState::Ready(fields[0].clone())
                    }
                }
                "Pending" => PollState::Pending,
                _ => PollState::NotAPoll,
            }
        }

        // Struct-based Poll (struct Poll_Ready { value: T } or struct Poll_Pending {})
        Value::Struct(name, fields) => {
            if name.contains("Ready") {
                let fields_guard = fields.read().unwrap();
                if let Some(val) = fields_guard
                    .get("0")
                    .or(fields_guard.get("value"))
                    .or(fields_guard.values().next())
                {
                    PollState::Ready(val.clone())
                } else {
                    PollState::Ready(Value::Unit)
                }
            } else if name.contains("Pending") {
                PollState::Pending
            } else {
                PollState::NotAPoll
            }
        }

        // Tuple style: ("Ready", value) or ("Pending",)
        Value::Tuple(elems) if elems.len() >= 1 => {
            if let Value::String(tag) = &elems[0] {
                match tag.as_str() {
                    "Ready" if elems.len() >= 2 => PollState::Ready(elems[1].clone()),
                    "Ready" => PollState::Ready(Value::Unit),
                    "Pending" => PollState::Pending,
                    _ => PollState::NotAPoll,
                }
            } else {
                PollState::NotAPoll
            }
        }

        _ => PollState::NotAPoll,
    }
}

/// Normalize various poll representations to our standard Value::Poll
fn normalize_poll_result(val: Value) -> Value {
    match extract_poll_result(&val) {
        PollState::Ready(inner) => Value::Poll(true, Some(Box::new(inner))),
        PollState::Pending => Value::Poll(false, None),
        PollState::NotAPoll => val, // Keep as-is
    }
}
