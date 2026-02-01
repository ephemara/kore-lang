// ============================================================================
// KORE Compiler - Self-Hosted Bootstrap Edition
// ============================================================================
// Project Ouroboros - The Snake That Eats Its Own Tail
//
// This is the first self-hosted Kore compiler!
// Generated from Kore source code, running as native Rust.
//
// Usage:
//   korec <source.kr>                    # Compile to LLVM IR (default)
//   korec <source.kr> --target llvm      # Compile to LLVM IR
//   korec <source.kr> --target rust      # Transpile to Rust
//   korec <source.kr> -o output.ll       # Specify output file
//
// ============================================================================

#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(dead_code)]
#![allow(unused_parens)]
#![allow(unused_imports)]

pub mod compiler;

use std::env;
use std::fs;
use std::path::Path;

use compiler::{Lexer, Parser, RustGen, LLVMGen};

fn main() {
    let args: Vec<String> = env::args().collect();
    
    println!(" Kore Compiler v0.2.0 (LLVM Bootstrap)");
    println!("Project Ouroboros - Native Compilation");
    eprintln!("DEBUG: Kore Compiler Starting... (Build Verification ID: 2980)");
    println!();
    
    if args.len() < 2 {
        println!("Usage: korec <source.kr> [--target llvm|rust] [-o output]");
        println!();
        println!("Targets:");
        println!("  llvm   - Generate LLVM IR (.ll) - compile with: clang output.ll -o output");
        println!("  rust   - Transpile to Rust (.rs) - compile with: rustc output.rs");
        println!();
        println!("This is the self-hosted Kore compiler!");
        return;
    }
    
    let source_file = &args[1];
    
    // Parse optional arguments
    let mut output_file: Option<String> = None;
    let mut target = "llvm";  // Default to LLVM now!
    
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--target" => {
                if i + 1 < args.len() {
                    target = &args[i + 1];
                    i += 1;
                }
            }
            "-o" => {
                if i + 1 < args.len() {
                    output_file = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    
    println!("Compiling: {}", source_file);
    println!("Target: {}", target);
    
    // Read source file
    let source = match fs::read_to_string(source_file) {
        Ok(content) => content,
        Err(e) => {
            println!(" Error reading file: {}", e);
            return;
        }
    };
    
    println!("Read {} bytes", source.len());
    
    // Lexing
    println!();
    println!(" Lexing...");
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize();
    println!("   {} tokens", tokens.len());
    
    // Parsing
    println!();
    println!(" Parsing...");
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program();
    println!("   {} items", program.items.len());
    
    // Code generation based on target
    let mut verify_ok_flag = true;
    
    // Determine output path first
    let out_ext = if target == "rust" || target == "rs" { "rs" } else { "ll" };
    let out_path = output_file.clone().unwrap_or_else(|| {
        Path::new(source_file)
            .file_stem()
            .map(|s| format!("{}.{}", s.to_string_lossy(), out_ext))
            .unwrap_or_else(|| format!("output.{}", out_ext))
    });
    
    if target == "llvm" || target == "ll" {
        println!();
        println!("Generating LLVM IR...");
        // Initialize Inkwell context
        let context = inkwell::context::Context::create();
        let mut gen = LLVMGen::new(&context, "kore_main");
        
        let success = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| gen.gen_program(program))) {
            Ok(_) => true,
            Err(_) => {
                println!(" Panic during IR generation");
                false
            }
        };
        
        if success {
             // CRITICAL: Write IR *before* verification because module.verify() can crash on invalid IR
             // This ensures we at least have the IR to debug.
             match gen.write_to_file(Path::new(&out_path)) {
                Ok(_) => println!(" Wrote to {}", out_path),
                Err(e) => {
                    println!(" Error writing file: {}", e);
                    std::process::exit(1);
                }
             }

             if gen.verify() {
                 println!("VERIFICATION: PASS");
             } else {
                 println!("VERIFICATION: FAIL");
                 verify_ok_flag = false;
             }
        } else {
            verify_ok_flag = false;
            // Attempt to write partial IR if possible? 
            // The module might be in a bad state, but try anyway
             match gen.write_to_file(Path::new(&out_path)) {
                Ok(_) => println!(" Wrote partial output to {}", out_path),
                Err(_) => {}
            }
        }
        
        // Write done above

        
    } else if target == "rust" || target == "rs" {
        println!();
        println!("  Generating Rust code...");
        let mut gen = RustGen::new();
        let code = gen.gen_program(program);
        println!("   {} characters", code.len());
        
        match fs::write(&out_path, &code) {
            Ok(_) => println!(" Wrote to {}", out_path),
            Err(e) => {
                println!(" Error writing file: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        println!(" Unknown target: {}. Use 'llvm' or 'rust'", target);
        return;
    }
    
    // Verify file exists (sanity check)
    if Path::new(&out_path).exists() {
        println!("DEBUG: File verification: EXISTS");
        if let Ok(metadata) = fs::metadata(&out_path) {
            println!("DEBUG: File size: {} bytes", metadata.len());
        }
    } else {
        println!("DEBUG: File verification: NOT FOUND after write!");
    }
    
    println!();
    println!(" Compilation complete!");
    println!();
    
    if target == "llvm" || target == "ll" {
        println!("Next steps:");
        println!("  1. Compile runtime: clang -c runtime/kore_runtime.c -o kore_runtime.o");
        println!("  2. Link: clang {} kore_runtime.o -o output.exe", out_path);
        println!("  3. Run: ./output.exe");
    } else {
        println!("Next steps:");
        println!("  1. rustc {} -o output.exe", out_path);
        println!("  2. ./output.exe");
    }
    // Exit code policy:
    // - If verification passed, exit 0
    // - If verification failed, still emit IR and exit 1
    if target == "llvm" || target == "ll" {
        if verify_ok_flag { std::process::exit(0); } else { std::process::exit(1); }
    }
}
