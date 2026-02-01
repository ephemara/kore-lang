
import os

SOURCES = [
    "src/span.kr",
    "src/error.kr", 
    "src/effects.kr",
    "src/ast.kr",
    "src/stdlib.kr",
    "src/types.kr",
    "src/lexer.kr",
    "src/lexer_static.kr",
    "src/parser.kr",
    "src/codegen.kr",
    "src/korec.kr"
]

OUTPUT = "korec_build_v2.kr"

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
