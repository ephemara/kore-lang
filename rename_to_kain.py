#!/usr/bin/env python3
"""
KAIN -> KAIN Mass Rename Script v2
More aggressive - catches KainResult, KainError inside .kn files
"""

import os
import re
from pathlib import Path

SKIP_DIRS = {'.git', 'target', 'node_modules', '__pycache__', '.vscode'}

TEXT_EXTENSIONS = {
    '.rs', '.toml', '.md', '.txt', '.json', '.yaml', '.yml', 
    '.py', '.sh', '.ps1', '.bat', '.cmd', '.kn', '.kn',
    '.html', '.css', '.js', '.ts', '.tsx', '.jsx',
    '.lock', '.gitignore', '.gitattributes'
}

def should_process_file(path: Path) -> bool:
    if path.suffix.lower() in {'.exe', '.dll', '.so', '.wasm', '.spv', '.png', '.jpg', '.gif', '.ico'}:
        return False
    if path.suffix.lower() in TEXT_EXTENSIONS:
        return True
    if path.suffix == '' and path.is_file():
        return True
    return False

def replace_in_content(content: str) -> tuple[str, int]:
    changes = 0
    
    # Direct string replacements for common patterns
    direct_replacements = [
        ('KainResult', 'KainResult'),
        ('KainError', 'KainError'),
        ('KAIN', 'KAIN'),
        ('Kain', 'Kain'),
        ('kain', 'kain'),
        ('.kn"', '.kn"'),
        (".kn'", ".kn'"),
        ('.kn)', '.kn)'),
        ('.kn,', '.kn,'),
        ('.kn ', '.kn '),
        ('.kr\n', '.kn\n'),
        ('.kr\t', '.kn\t'),
    ]
    
    new_content = content
    for old, new in direct_replacements:
        if old in new_content:
            count = new_content.count(old)
            new_content = new_content.replace(old, new)
            changes += count
    
    return new_content, changes

def process_file(filepath: Path) -> int:
    try:
        content = filepath.read_text(encoding='utf-8', errors='ignore')
        new_content, changes = replace_in_content(content)
        
        if changes > 0:
            filepath.write_text(new_content, encoding='utf-8')
            print(f"  [{changes:3d} changes] {filepath}")
        
        return changes
    except Exception as e:
        print(f"  [ERROR] {filepath}: {e}")
        return 0

def main():
    root = Path('.')
    total_files = 0
    total_changes = 0
    
    print("=" * 60)
    print("KAIN -> KAIN Mass Rename (Round 2)")
    print("=" * 60)
    
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
        
        for filename in filenames:
            filepath = Path(dirpath) / filename
            
            if should_process_file(filepath):
                changes = process_file(filepath)
                if changes > 0:
                    total_files += 1
                    total_changes += changes
    
    print("=" * 60)
    print(f"DONE: {total_changes} replacements in {total_files} files")
    print("=" * 60)

if __name__ == '__main__':
    main()
