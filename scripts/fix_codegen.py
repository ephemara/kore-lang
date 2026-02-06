
import re

file_path = 'K:/KAIN/src/codegen.kn'

with open(file_path, 'r', encoding='utf-8') as f:
    content = f.read()

# Helper to replace using regex
def replace_re(pattern, replacement, text):
    return re.sub(pattern, replacement, text)

# Replace v == "..." with str_eq(v, "...")
# Regex is better for whitespace
# Pattern: \b(\w+)\s*==\s*"([^"]+)"
# Replacement: str_eq(\1, "\2")

content = replace_re(r'\b([a-zA-Z0-9_]+)\s*==\s*"([^"]+)"', r'str_eq(\1, "\2")', content)

# Also !=
content = replace_re(r'\b([a-zA-Z0-9_]+)\s*!=\s*"([^"]+)"', r'!str_eq(\1, "\2")', content)

# Also variant_of(...) == "..."
content = replace_re(r'variant_of\(([^)]+)\)\s*==\s*"([^"]+)"', r'str_eq(variant_of(\1), "\2")', content)
content = replace_re(r'variant_of\(([^)]+)\)\s*!=\s*"([^"]+)"', r'!str_eq(variant_of(\1), "\2")', content)

# Special cases with variables on both sides
# f == field_name
content = replace_re(r'\bf\s*==\s*field_name\b', r'str_eq(f, field_name)', content)
content = replace_re(r'\bf\s*==\s*init_name\b', r'str_eq(f, init_name)', content)

# fields_str == ""
content = replace_re(r'fields_str\s*==\s*""', r'str_eq(fields_str, "")', content)

# last_was_return = ...
# (v == "Return") is handled by general regex
# (v == "7") is handled by general regex (digits allowed in pattern)

with open(file_path, 'w', encoding='utf-8') as f:
    f.write(content)

print("Replaced patterns in " + file_path)
