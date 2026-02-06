"""
Emoji Removal Script for KAIN Language
Strips all emoji characters from source files to ensure cross-platform compilation compatibility.
"""

import os
import re
import sys
from pathlib import Path

# Comprehensive emoji regex pattern covering all major Unicode emoji ranges
EMOJI_PATTERN = re.compile(
    "["
    "\U0001F300-\U0001F9FF"  # Miscellaneous Symbols and Pictographs, Emoticons, etc.
    "\U00002600-\U000026FF"  # Misc symbols (sun, clouds, etc.)
    "\U00002700-\U000027BF"  # Dingbats
    "\U0001FA00-\U0001FA6F"  # Chess Symbols
    "\U0001FA70-\U0001FAFF"  # Symbols and Pictographs Extended-A
    "\U00002300-\U000023FF"  # Miscellaneous Technical
    "\U0000FE00-\U0000FE0F"  # Variation Selectors
    "\U0001F000-\U0001F02F"  # Mahjong Tiles
    "\U0001F0A0-\U0001F0FF"  # Playing Cards
    "]+",
    flags=re.UNICODE
)

# File extensions to process
TARGET_EXTENSIONS = {'.kn', '.rs', '.c', '.h', '.cpp', '.hpp', '.md', '.ll', '.txt'}

# Directories to skip entirely
SKIP_DIRS = {'.git', 'node_modules', 'target', '.vs', '__pycache__', 'scripts'}

def remove_emojis_from_text(text: str) -> tuple[str, int]:
    """
    Remove all emoji characters from the given text.
    Returns the cleaned text and the count of emojis removed.
    """
    matches = EMOJI_PATTERN.findall(text)
    emoji_count = sum(len(m) for m in matches)
    cleaned = EMOJI_PATTERN.sub('', text)
    return cleaned, emoji_count

def process_file(filepath: Path, dry_run: bool = False) -> tuple[int, bool]:
    """
    Process a single file, removing emojis.
    Returns (emoji_count, was_modified).
    """
    try:
        content = filepath.read_text(encoding='utf-8')
    except UnicodeDecodeError:
        # Try with latin-1 as fallback for binary-ish files
        try:
            content = filepath.read_text(encoding='latin-1')
        except Exception:
            return 0, False
    except Exception as e:
        print(f"  [SKIP] Could not read {filepath}: {e}")
        return 0, False
    
    cleaned, emoji_count = remove_emojis_from_text(content)
    
    if emoji_count > 0:
        if dry_run:
            print(f"  [DRY-RUN] Would remove {emoji_count} emoji(s) from {filepath}")
        else:
            filepath.write_text(cleaned, encoding='utf-8')
            print(f"  [CLEANED] Removed {emoji_count} emoji(s) from {filepath}")
        return emoji_count, True
    
    return 0, False

def walk_directory(root_path: Path, dry_run: bool = False) -> dict:
    """
    Walk through the directory tree and process all matching files.
    Returns statistics about the operation.
    """
    stats = {
        'files_scanned': 0,
        'files_modified': 0,
        'total_emojis_removed': 0,
        'errors': []
    }
    
    for dirpath, dirnames, filenames in os.walk(root_path):
        # Filter out directories we want to skip
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
        
        for filename in filenames:
            filepath = Path(dirpath) / filename
            
            # Only process files with target extensions
            if filepath.suffix.lower() not in TARGET_EXTENSIONS:
                continue
            
            stats['files_scanned'] += 1
            
            try:
                emoji_count, was_modified = process_file(filepath, dry_run)
                if was_modified:
                    stats['files_modified'] += 1
                    stats['total_emojis_removed'] += emoji_count
            except Exception as e:
                stats['errors'].append((str(filepath), str(e)))
    
    return stats

def main():
    # Determine the root directory (script location's parent)
    if len(sys.argv) > 1:
        root = Path(sys.argv[1])
    else:
        root = Path(__file__).parent.parent
    
    # Check for dry-run flag
    dry_run = '--dry-run' in sys.argv or '-n' in sys.argv
    
    print("=" * 60)
    print("KAIN Emoji Removal Tool")
    print("=" * 60)
    print(f"Root directory: {root.absolute()}")
    print(f"Mode: {'DRY-RUN (no changes will be made)' if dry_run else 'LIVE (files will be modified)'}")
    print(f"Target extensions: {', '.join(sorted(TARGET_EXTENSIONS))}")
    print("-" * 60)
    
    if not root.exists():
        print(f"ERROR: Directory does not exist: {root}")
        sys.exit(1)
    
    stats = walk_directory(root, dry_run)
    
    print("-" * 60)
    print("SUMMARY")
    print("-" * 60)
    print(f"Files scanned:    {stats['files_scanned']}")
    print(f"Files modified:   {stats['files_modified']}")
    print(f"Emojis removed:   {stats['total_emojis_removed']}")
    
    if stats['errors']:
        print(f"\nErrors encountered: {len(stats['errors'])}")
        for filepath, error in stats['errors']:
            print(f"  - {filepath}: {error}")
    
    print("=" * 60)
    
    if dry_run:
        print("\nRun without --dry-run to apply changes.")

if __name__ == "__main__":
    main()
