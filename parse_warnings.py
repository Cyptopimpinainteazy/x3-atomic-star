import re
from collections import defaultdict

def parse_warnings(file_path):
    with open(file_path, 'r', encoding='utf-8', errors='ignore') as f:
        lines = f.readlines()

    warnings_by_file = defaultdict(list)
    
    for i, line in enumerate(lines):
        # Look for the file indicator
        path_match = re.search(r'^\s+-->\s+([^:]+):(\d+):(\d+)', line)
        if path_match:
            file_path_found = path_match.group(1)
            line_num = path_match.group(2)
            
            # Find the message above the path
            message = None
            is_warning = False
            for j in range(i-1, max(-1, i-5), -1):
                if lines[j].startswith('warning: '):
                    if 'generated' in lines[j] and 'warning' in lines[j]:
                        continue
                    message = lines[j].strip()
                    is_warning = True
                    break
                elif lines[j].startswith('error'):
                    # It's an error, we only want warnings
                    is_warning = False
                    break
            
            if not is_warning:
                continue

            # Extract context
            context = [message, line.strip()]
            k = i + 1
            while k < len(lines):
                l = lines[k].rstrip()
                # Stop if we hit a new diagnostic or empty line that isn't part of context
                if 'warning:' in l or 'error:' in l or '-->' in l:
                    break
                
                # Check for context patterns: " |", "123 |", "help:", etc.
                if re.match(r'^\s*(\d+)?\s*\|', l) or 'help:' in l or 'suggestion' in l or '^^^^' in l or '----' in l or l.strip().startswith('='):
                    context.append(l)
                elif l.strip() == '':
                    # Peek next line to see if context continues
                    if k+1 < len(lines) and (re.match(r'^\s*(\d+)?\s*\|', lines[k+1]) or 'help:' in lines[k+1].strip()):
                        context.append(l)
                    else:
                        break
                else:
                    break
                k += 1
            
            warnings_by_file[file_path_found].append({
                'line': line_num,
                'message': message,
                'context': "\n".join(context)
            })

    # Deduplicate by context within each file
    for file in warnings_by_file:
        seen = set()
        unique = []
        for w in warnings_by_file[file]:
            if w['context'] not in seen:
                unique.append(w)
                seen.add(w['context'])
        warnings_by_file[file] = unique

    return warnings_by_file

if __name__ == "__main__":
    log_file = '/tmp/x3_check_all_targets.txt'
    warnings = parse_warnings(log_file)
    
    files = sorted(warnings.keys())
    if not files:
        print("No warnings found.")
    else:
        for file in files:
            print(f"File: {file}")
            print("-" * len(f"File: {file}"))
            for w in warnings[file]:
                print(w['context'])
                print()
            print("=" * 40)
