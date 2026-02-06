import sys

def patch(filename):
    with open(filename, 'r') as f:
        lines = f.readlines()
    
    new_lines = []
    skip = False
    
    # Declarations to add at the top (after header comments)
    inserted = False
    
    for line in lines:
        # Insert declarations early
        if not inserted and line.strip() == "":
            new_lines.append("declare i64 @kain_create_token_simple(i64)\n")
            new_lines.append("declare i64 @kain_create_token_payload(i64, i64)\n")
            inserted = True
            
        if line.startswith("define i64 @kain_create_token_simple"):
            skip = True
        elif line.startswith("define i64 @kain_create_token_payload"):
            skip = True
        
        if skip:
            if line.strip() == "}":
                skip = False
            continue
            
        new_lines.append(line)
            
    with open(filename, 'w') as f:
        f.writelines(new_lines)

if __name__ == "__main__":
    patch(sys.argv[1])