//! KAIN Compiler CLI

use clap::Parser as ClapParser;
use std::path::PathBuf;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use kain::{compile, CompileTarget, VERSION, LANGUAGE_NAME};
use kain::packager;
use kain::lsp;

#[derive(ClapParser, Debug)]
#[command(name = "kain")]
#[command(author = "Kipp")]
#[command(version = VERSION)]
#[command(about = "The Ultimate Programming Language Compiler", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Source file to compile (legacy positional argument)
    input: Option<PathBuf>,

    /// Output file
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Compilation target
    #[arg(short, long, default_value = "wasm")]
    target: String,

    /// Run immediately after compilation
    #[arg(short, long)]
    run: bool,

    /// Watch for file changes and recompile
    #[arg(short, long)]
    watch: bool,

    /// Emit AST for debugging  
    #[arg(long)]
    emit_ast: bool,

    /// Emit typed AST
    #[arg(long)]
    emit_typed: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Target plugin name for UE5 shader copy
    #[arg(long)]
    plugin: Option<String>,

    /// Base plugins directory (defaults to u:\ue_factory\src-plugins)
    #[arg(long)]
    plugins_dir: Option<PathBuf>,

    /// Print planned actions without executing
    #[arg(long)]
    dry_run: bool,

    /// Treat transpiler warnings as errors when supported
    #[arg(long)]
    strict: bool,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Initialize a new KAIN project
    Init {
        /// Project name
        #[arg(default_value = ".")]
        path: PathBuf,
        
        /// Explicit project name
        #[arg(long)]
        name: Option<String>,
    },
    
    /// Start the Language Server
    Lsp,

    /// Compile a file (explicit command)
    Build {
        input: PathBuf,
    },
    
    /// Run a file (explicit command)
    Run {
        input: PathBuf,
    }
}

fn run_compile(input: &PathBuf, target: CompileTarget, output: Option<&PathBuf>, _emit_ast: bool, _emit_typed: bool, verbose: bool) -> bool {
    // Read source
    let source = match fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(" Failed to read {}: {}", input.display(), e);
            return false;
        }
    };

    if verbose {
        println!(" Compiling: {}", input.display());
        println!(" Source: {} bytes, {} lines", source.len(), source.lines().count());
    }

    // Compile
    match compile(&source, target) {
        Ok(compiled_output) => {
            if target == CompileTarget::Interpret || target == CompileTarget::Test {
                println!(" Execution complete");
            } else {
                let default_ext = match target {
                    CompileTarget::Wasm => "wasm",
                    CompileTarget::Llvm => "ll",
                    CompileTarget::SpirV => "spv",
                    CompileTarget::Hlsl => "hlsl",
                    CompileTarget::Usf => "usf",
                    CompileTarget::Js => "js",
                    CompileTarget::Rust => "rs",
                    CompileTarget::Interpret | CompileTarget::Test => unreachable!(),
                };
                
                // Determine where to write the primary output (IR for LLVM, Binary for others)
                let output_path = if target == CompileTarget::Llvm {
                    // For LLVM, we always write the IR file first
                    // If user specified -o main.exe, we write to main.ll
                    if let Some(out) = output {
                        if out.extension().map_or(false, |e| e == "ll") {
                            out.clone()
                        } else {
                            // Strip extension and add .ll, or append .ll
                            let mut p = out.clone();
                            p.set_extension("ll");
                            p
                        }
                    } else {
                        input.with_extension("ll")
                    }
                } else {
                    output.cloned().unwrap_or_else(|| {
                        input.with_extension(default_ext)
                    })
                };
                
                if let Err(e) = fs::write(&output_path, &compiled_output) {
                    eprintln!(" Failed to write output: {}", e);
                    return false;
                }
                
                println!(" Compiled to: {} ({} bytes)", output_path.display(), compiled_output.len());

                // Post-processing for LLVM
                if target == CompileTarget::Llvm {
                    let exe_path = output.cloned().unwrap_or_else(|| {
                        if cfg!(windows) {
                            input.with_extension("exe")
                        } else {
                            input.with_extension("")
                        }
                    });

                    println!(" Linking executable...");
                    
                    // Try to find clang
                    let clang_cmd = if std::process::Command::new("clang").arg("--version").output().is_ok() {
                        "clang".to_string()
                    } else {
                        let default_path = r"C:\Program Files\LLVM\bin\clang.exe";
                        if std::path::Path::new(default_path).exists() {
                            default_path.to_string()
                        } else {
                            "clang".to_string()
                        }
                    };

                    let mut cmd = std::process::Command::new(&clang_cmd);

                    // Compile and link Runtime Library
                    let runtime_c = std::path::Path::new("src/runtime/c/KAIN_runtime.c");
                    if runtime_c.exists() {
                         let runtime_o = if cfg!(windows) { "src/runtime/c/KAIN_runtime.obj" } else { "src/runtime/c/KAIN_runtime.o" };
                         
                         let status = std::process::Command::new(&clang_cmd)
                             .arg("-c")
                             .arg(runtime_c)
                             .arg("-o")
                             .arg(runtime_o)
                             .status();
                             
                         if let Ok(s) = status {
                             if s.success() {
                                 cmd.arg(runtime_o);
                             } else {
                                 eprintln!(" Failed to compile runtime library.");
                             }
                         }
                    }

                    cmd.arg(&output_path)
                        .arg("-o")
                        .arg(&exe_path)
                        .arg("-Wno-override-module")
                        .arg("-g"); // Debug info

                    if cfg!(windows) {
                        cmd.arg("-llegacy_stdio_definitions");
                    }

                    let status = cmd.status();

                    match status {
                        Ok(s) if s.success() => {
                            println!(" Generated executable: {}", exe_path.display());
                        },
                        Ok(_) => {
                            eprintln!(" Linking failed."); 
                        },
                        Err(_) => {
                            eprintln!(" 'clang' not found in PATH or standard locations.");
                            eprintln!("   To generate an executable, install LLVM and run:");
                            eprintln!("   clang {} -o {}", output_path.display(), exe_path.display());
                        }
                    }
                }
            }
            true
        }
        Err(e) => {
            // Use pretty error formatting
            let filename = input.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("input.kn");
            let diag = kain::diagnostics::Diagnostics::new(&source, filename);
            eprint!("{}", diag.format_error(&e));
            false
        }
    }
}

fn watch_mode(input: PathBuf, target: CompileTarget, output: Option<PathBuf>, emit_ast: bool, emit_typed: bool, verbose: bool) {
    use notify::{Watcher, RecursiveMode, Event};
    use std::sync::mpsc::channel;
    
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    
    ctrlc::set_handler(move || {
        println!("\n Stopping watch mode...");
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");
    
    println!(" Watching {} for changes... (Ctrl+C to stop)", input.display());
    println!("");
    
    // Initial compile
    run_compile(&input, target, output.as_ref(), emit_ast, emit_typed, verbose);
    println!("");
    
    let (tx, rx) = channel();
    
    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            if event.kind.is_modify() {
                let _ = tx.send(());
            }
        }
    }).expect("Failed to create watcher");
    
    watcher.watch(&input, RecursiveMode::NonRecursive).expect("Failed to watch file");
    
    // Also watch parent directory in case file is replaced
    if let Some(parent) = input.parent() {
        let _ = watcher.watch(parent, RecursiveMode::NonRecursive);
    }
    
    while running.load(Ordering::SeqCst) {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(_) => {
                // Debounce - wait a bit for writes to settle
                std::thread::sleep(Duration::from_millis(50));
                // Drain any pending events
                while rx.try_recv().is_ok() {}
                
                println!(" File changed, recompiling...");
                println!("");
                run_compile(&input, target, output.as_ref(), emit_ast, emit_typed, verbose);
                println!("");
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Keep looping
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }
}

fn main() {
    let builder = std::thread::Builder::new()
        .name("main-thread".into())
        .stack_size(8 * 1024 * 1024); // 8MB

    let handler = builder.spawn(|| {
        let args = Args::parse();

        println!(" {} Compiler v{}", LANGUAGE_NAME, VERSION);

        match args.command {
            Some(Commands::Init { path, name }) => {
                if let Err(e) = packager::init_project(&path, name) {
                    eprintln!(" Init failed: {}", e);
                }
            }
            Some(Commands::Lsp) => {
                eprintln!(" Starting KAIN Language Server...");
                // Manual runtime for LSP
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to create tokio runtime");
                
                rt.block_on(async {
                    lsp::run_server().await;
                });
            }
            Some(Commands::Build { input }) => {
                run_compile(&input, CompileTarget::Wasm, None, args.emit_ast, args.emit_typed, args.verbose);
            }
            Some(Commands::Run { input }) => {
                run_compile(&input, CompileTarget::Interpret, None, args.emit_ast, args.emit_typed, args.verbose);
            }
            None => {
                // Legacy behavior
                if let Some(ref input) = args.input {
                    if args.target.as_str() == "ue5-shader" {
                        if args.watch {
                            eprintln!(" Watch mode is not supported for ue5-shader target.");
                        }
                        if !run_ue5_shader_pipeline(&input, &args) {
                            std::process::exit(1);
                        }
                    } else {
                        let target = match args.target.as_str() {
                            "wasm" | "w" => CompileTarget::Wasm,
                            "llvm" | "native" | "n" => CompileTarget::Llvm,
                            "spirv" | "gpu" | "shader" | "s" => CompileTarget::SpirV,
                            "hlsl" | "h" => CompileTarget::Hlsl,
                            "usf" | "ue5" => CompileTarget::Usf,
                            "js" | "javascript" => CompileTarget::Js,
                            "rust" | "rs" => CompileTarget::Rust,
                            "run" | "r" | "interpret" | "i" => CompileTarget::Interpret,
                            "test" | "t" => CompileTarget::Test,
                            _ => {
                                eprintln!(" Unknown target: {}. Use: wasm, llvm, spirv, hlsl, usf, js, rust, run, test, or ue5-shader", args.target);
                                std::process::exit(1);
                            }
                        };

                        if args.watch {
                            watch_mode(input.clone(), target, args.output.clone(), args.emit_ast, args.emit_typed, args.verbose);
                        } else {
                            if !run_compile(&input, target, args.output.as_ref(), args.emit_ast, args.emit_typed, args.verbose) {
                                std::process::exit(1);
                            }
                        }
                    }
                } else {
                    eprintln!(" No input file provided. Use --help for usage.");
                }
            }
        }
    }).unwrap();

    handler.join().unwrap();
}

fn staging_dir() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let candidate1 = cwd.join("src-plugins");
    if candidate1.exists() {
        return candidate1.join("_shaders");
    }
    if let Some(parent) = cwd.parent() {
        let candidate2 = parent.join("src-plugins");
        if candidate2.exists() {
            return candidate2.join("_shaders");
        }
    }
    cwd.join("src-plugins").join("_shaders")
}

fn ensure_dir(p: &PathBuf) -> bool {
    if let Err(e) = fs::create_dir_all(p) {
        eprintln!(" Failed to create directory {}: {}", p.display(), e);
        return false;
    }
    true
}

fn find_binary(name: &str, fallback: Option<&str>) -> Option<PathBuf> {
    if std::process::Command::new(name).arg("--version").output().is_ok() {
        return Some(PathBuf::from(name));
    }
    if let Some(f) = fallback {
        let pb = PathBuf::from(f);
        if pb.exists() {
            return Some(pb);
        }
    }
    None
}

fn derive_shader_paths(input: &PathBuf) -> (PathBuf, PathBuf, PathBuf) {
    let stage = staging_dir();
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("shader");
    let spv = stage.join(format!("{}.spv", stem));
    let hlsl = stage.join(format!("{}.hlsl", stem));
    let usf = stage.join(format!("{}.usf", stem));
    (spv, hlsl, usf)
}

fn resolve_plugin_dir(plugin: &str, base_opt: &Option<PathBuf>) -> PathBuf {
    if let Some(base) = base_opt {
        return base.join(plugin).join("Shaders");
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let candidate1 = cwd.join("src-plugins");
    if candidate1.exists() {
        return candidate1.join(plugin).join("Shaders");
    }
    if let Some(parent) = cwd.parent() {
        let candidate2 = parent.join("src-plugins");
        if candidate2.exists() {
            return candidate2.join(plugin).join("Shaders");
        }
    }
    cwd.join("src-plugins").join(plugin).join("Shaders")
}

fn run_ue5_shader_pipeline(input: &PathBuf, args: &Args) -> bool {
    let (spv_path, hlsl_path, usf_path) = derive_shader_paths(input);
    let stage_dir = staging_dir();
    if !ensure_dir(&stage_dir) {
        return false;
    }

    let source = match fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(" Failed to read {}: {}", input.display(), e);
            return false;
        }
    };

    if args.verbose {
        println!(" Compiling: {}", input.display());
    }

    let compiled_spv = match compile(&source, CompileTarget::SpirV) {
        Ok(bytes) => bytes,
        Err(e) => {
            let filename = input.file_name().and_then(|s| s.to_str()).unwrap_or("input.kn");
            let diag = kain::diagnostics::Diagnostics::new(&source, filename);
            eprint!("{}", diag.format_error(&e));
            return false;
        }
    };

    if args.dry_run {
        println!("→ Write SPIR-V {}", spv_path.display());
    } else if let Err(e) = fs::write(&spv_path, &compiled_spv) {
        eprintln!(" Failed to write {}: {}", spv_path.display(), e);
        return false;
    } else {
        if args.verbose {
            println!(" {}", spv_path.display());
        }
    }

    if let Some(val_bin) = find_binary("spirv-val", None) {
        if args.verbose {
            println!(" Validating SPIR-V");
        }
        if !args.dry_run {
            let status = std::process::Command::new(val_bin)
                .arg(&spv_path)
                .status();
            if let Ok(s) = status {
                if !s.success() {
                    eprintln!(" SPIR-V validation failed");
                    return false;
                }
            }
        }
    }

    let naga_bin = match find_binary("naga", None) {
        Some(p) => p,
        None => {
            eprintln!(" 'naga' not found. Install with: cargo install naga-cli");
            return false;
        }
    };

    if args.verbose {
        println!(" Transpiling to HLSL");
    }
    if args.dry_run {
        println!("→ Run naga {} {}", spv_path.display(), hlsl_path.display());
    } else {
        let status = std::process::Command::new(naga_bin)
            .arg(&spv_path)
            .arg(&hlsl_path)
            .status();
        match status {
            Ok(s) if s.success() => {
                if args.verbose {
                    println!(" {}", hlsl_path.display());
                }
            }
            _ => {
                eprintln!(" Naga transpilation failed");
                return false;
            }
        }
    }

    if args.dry_run {
        println!("→ Write USF {}", usf_path.display());
    } else {
        let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("shader");
        let content = format!("#include \"{}.hlsl\"\n", stem);
        if let Err(e) = fs::write(&usf_path, content) {
            eprintln!(" Failed to write {}: {}", usf_path.display(), e);
            return false;
        }
    }

    if let Some(plugin) = &args.plugin {
        let target_dir = resolve_plugin_dir(plugin, &args.plugins_dir);
        if args.dry_run {
            println!("→ Copy to {}", target_dir.display());
        } else {
            if !ensure_dir(&target_dir.clone()) {
                return false;
            }
            let hlsl_target = target_dir.join(hlsl_path.file_name().unwrap());
            let usf_target = target_dir.join(usf_path.file_name().unwrap());
            if let Err(e) = fs::copy(&hlsl_path, &hlsl_target) {
                eprintln!(" Copy failed: {}", e);
                return false;
            }
            if let Err(e) = fs::copy(&usf_path, &usf_target) {
                eprintln!(" Copy failed: {}", e);
                return false;
            }
            println!(" {}", target_dir.display());
        }
    } else {
        if args.verbose {
            println!(" Staged in {}", stage_dir.display());
        }
    }

    true
}

