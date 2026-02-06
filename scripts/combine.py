
import os

SOURCES = [
    "src/span.kn",
    "src/error.kn", 
    "src/effects.kn",
    "src/ast.kn",
    "src/stdlib.kn",
    "src/types.kn",
    "src/lexer.kn",
    "src/lexer_static.kn",
    "src/parser.kn",
    "src/codegen.kn",
    "src/kainc.kn"
]

OUTPUT = "kainc_build_v2.kn"

SKIP_MODULES = {"span", "error", "effects", "ast", "stdlib", "types", "lexer", "lexer_static", "parser", "codegen"}

def combine():
    with open(OUTPUT, 'w') as out:
        out.write("// Combined Source\n\n")
        
        for src in SOURCES:
            print(f"Processing {src}...")
            out.write(f"\n// ======== {src} ========\n\n")
            
            with open(src, 'r') as f:
                lines = f.readlines()
                
            for line in lines:
                stripped = line.strip()
                if stripped.startswith("use "):
                    module = stripped.split()[1]
                    if module in SKIP_MODULES:
                        out.write(f"// {line}") # Comment out
                        continue
                out.write(line)

if __name__ == "__main__":
    combine()
