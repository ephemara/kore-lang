//! KAIN Standard Library

use crate::types::ResolvedType;
use std::collections::HashMap;

/// Built-in function registry
pub struct StdLib {
    pub functions: HashMap<String, BuiltinFn>,
    pub types: HashMap<String, ResolvedType>,
}

pub struct BuiltinFn {
    pub name: &'static str,
    pub params: Vec<(&'static str, &'static str)>,
    pub return_type: &'static str,
    pub doc: &'static str,
}

impl StdLib {
    pub fn new() -> Self {
        let mut lib = Self {
            functions: HashMap::new(),
            types: HashMap::new(),
        };
        
        // I/O
        lib.add_fn("print", &[("value", "Any")], "Unit", "Print value to console");
        lib.add_fn("println", &[("value", "Any")], "Unit", "Print value with newline");
        lib.add_fn("read_line", &[], "String", "Read line from stdin");
        lib.add_fn("read_file", &[("path", "String")], "String", "Read file contents");
        lib.add_fn("write_file", &[("path", "String"), ("content", "String")], "Unit", "Write to file");
        
        // Math
        lib.add_fn("abs", &[("x", "Int")], "Int", "Absolute value");
        lib.add_fn("sqrt", &[("x", "Float")], "Float", "Square root");
        lib.add_fn("pow", &[("base", "Float"), ("exp", "Float")], "Float", "Power");
        lib.add_fn("sin", &[("x", "Float")], "Float", "Sine");
        lib.add_fn("cos", &[("x", "Float")], "Float", "Cosine");
        lib.add_fn("tan", &[("x", "Float")], "Float", "Tangent");
        lib.add_fn("floor", &[("x", "Float")], "Int", "Floor");
        lib.add_fn("ceil", &[("x", "Float")], "Int", "Ceiling");
        lib.add_fn("round", &[("x", "Float")], "Int", "Round");
        lib.add_fn("min", &[("a", "Int"), ("b", "Int")], "Int", "Minimum");
        lib.add_fn("max", &[("a", "Int"), ("b", "Int")], "Int", "Maximum");
        lib.add_fn("clamp", &[("x", "Int"), ("lo", "Int"), ("hi", "Int")], "Int", "Clamp between bounds");
        
        // Vector math (for shaders)
        lib.add_fn("vec2", &[("x", "Float"), ("y", "Float")], "Vec2", "Create 2D vector");
        lib.add_fn("vec3", &[("x", "Float"), ("y", "Float"), ("z", "Float")], "Vec3", "Create 3D vector");
        lib.add_fn("vec4", &[("x", "Float"), ("y", "Float"), ("z", "Float"), ("w", "Float")], "Vec4", "Create 4D vector");
        lib.add_fn("dot", &[("a", "Vec3"), ("b", "Vec3")], "Float", "Dot product");
        lib.add_fn("cross", &[("a", "Vec3"), ("b", "Vec3")], "Vec3", "Cross product");
        lib.add_fn("normalize", &[("v", "Vec3")], "Vec3", "Normalize vector");
        lib.add_fn("length", &[("v", "Vec3")], "Float", "Vector length");
        lib.add_fn("distance", &[("a", "Vec3"), ("b", "Vec3")], "Float", "Distance between points");
        lib.add_fn("mix", &[("a", "Float"), ("b", "Float"), ("t", "Float")], "Float", "Linear interpolation");
        lib.add_fn("smoothstep", &[("edge0", "Float"), ("edge1", "Float"), ("x", "Float")], "Float", "Smooth step");
        
        // Collections
        lib.add_fn("len", &[("collection", "Any")], "Int", "Get length");
        lib.add_fn("push", &[("array", "Array"), ("value", "Any")], "Unit", "Push to array");
        lib.add_fn("pop", &[("array", "Array")], "Any", "Pop from array");
        lib.add_fn("map", &[("array", "Array"), ("fn", "Function")], "Array", "Map over array");
        lib.add_fn("filter", &[("array", "Array"), ("fn", "Function")], "Array", "Filter array");
        lib.add_fn("reduce", &[("array", "Array"), ("initial", "Any"), ("fn", "Function")], "Any", "Reduce array");
        lib.add_fn("range", &[("start", "Int"), ("end", "Int")], "Array", "Create range");
        
        // HashMap
        lib.add_fn("map_new", &[], "Any", "Create new map");
        lib.add_fn("map_set", &[("map", "Any"), ("key", "String"), ("value", "Any")], "Unit", "Set map key");
        lib.add_fn("map_get", &[("map", "Any"), ("key", "String")], "Any", "Get map value");
        
        // Sockets
        lib.add_fn("socket_connect", &[("host", "String"), ("port", "Int")], "Int", "Connect TCP socket");
        lib.add_fn("socket_send", &[("sock", "Int"), ("data", "String")], "Unit", "Send data");
        lib.add_fn("socket_recv", &[("sock", "Int")], "String", "Receive data");
        
        // String
        lib.add_fn("split", &[("s", "String"), ("sep", "String")], "Array", "Split string");
        lib.add_fn("join", &[("arr", "Array"), ("sep", "String")], "String", "Join array to string");
        lib.add_fn("trim", &[("s", "String")], "String", "Trim whitespace");
        lib.add_fn("to_upper", &[("s", "String")], "String", "To uppercase");
        lib.add_fn("to_lower", &[("s", "String")], "String", "To lowercase");
        lib.add_fn("contains", &[("s", "String"), ("sub", "String")], "Bool", "Check contains");
        lib.add_fn("replace", &[("s", "String"), ("from", "String"), ("to", "String")], "String", "Replace substring");
        
        // Conversion
        lib.add_fn("to_string", &[("value", "Any")], "String", "Convert to string");
        lib.add_fn("to_int", &[("value", "Any")], "Int", "Convert to int");
        lib.add_fn("to_float", &[("value", "Any")], "Float", "Convert to float");
        
        // Debug
        lib.add_fn("dbg", &[("value", "Any")], "Any", "Debug print and return");
        lib.add_fn("assert", &[("condition", "Bool"), ("message", "String")], "Unit", "Assert condition");
        lib.add_fn("panic", &[("message", "String")], "Never", "Panic with message");
        
        // Time
        lib.add_fn("now", &[], "Float", "Current time in seconds");
        lib.add_fn("sleep", &[("seconds", "Float")], "Unit", "Sleep for seconds");
        
        // Actors
        lib.add_fn("spawn", &[("actor", "Actor")], "ActorRef", "Spawn actor");
        lib.add_fn("send", &[("actor", "ActorRef"), ("message", "Message")], "Unit", "Send message");
        
        // Python FFI
        lib.add_fn("py_eval", &[("code", "String")], "Any", "Evaluate Python expression");
        lib.add_fn("py_exec", &[("code", "String")], "Unit", "Execute Python code");
        lib.add_fn("py_import", &[("module", "String")], "Any", "Import Python module");

        // UI
        lib.add_fn("mount", &[("component", "Any"), ("selector", "String")], "Unit", "Mount component to DOM");

        lib
    }
    
    fn add_fn(&mut self, name: &'static str, params: &[(&'static str, &'static str)], ret: &'static str, doc: &'static str) {
        self.functions.insert(name.to_string(), BuiltinFn {
            name,
            params: params.to_vec(),
            return_type: ret,
            doc,
        });
    }
}

impl Default for StdLib {
    fn default() -> Self {
        Self::new()
    }
}

