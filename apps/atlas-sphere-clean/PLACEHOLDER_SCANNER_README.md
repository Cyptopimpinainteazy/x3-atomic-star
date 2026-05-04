# Placeholder Scanner for atlas-sphere

A comprehensive Python tool to scan repositories for code placeholders, TODOs, FIXMEs, template markers, and potential security issues.

## Features

- **31 pattern types** including TODO, FIXME, HACK, XXX, BUG, template markers (Mustache, ERB, Jinja), environment variables, and more
- **Severity classification** (CRITICAL, HIGH, MEDIUM, LOW, INFO) for prioritization
- **Binary file detection** - automatically skips images, executables, archives
- **Large file handling** - configurable size limits (default 5MB)
- **Smart directory skipping** - ignores `.git`, `node_modules`, `target`, `build`, etc.
- **Context lines** - captures surrounding code for each match
- **Multiple output formats**:
  - JSON (machine-readable, with statistics)
  - Human-readable report (with severity markers)
  - Summary-only mode for large repos
- **Cross-platform** - works on Linux, macOS, Windows
- **Zero dependencies** - uses only Python standard library

## Quick Start

```bash
# Scan current directory
python3 scan_placeholders.py --path . -o placeholders.json --report report.txt

# Scan a GitHub repo (clones to temp directory)
python3 scan_placeholders.py --url https://github.com/user/repo.git -o results.json

# Verbose mode with progress updates
python3 scan_placeholders.py --path /path/to/repo -v

# Summary only (no detailed context - good for large repos)
python3 scan_placeholders.py --path . --summary-only -o summary.json
```

## Command-Line Options

| Option | Description | Default |
|--------|-------------|---------|
| `--path PATH` | Path to local repository | Current directory |
| `--url URL` | Git clone URL (overrides path) | None |
| `--output`, `-o FILE` | Output JSON file path | `placeholders.json` |
| `--report`, `-r FILE` | Output readable report path | `placeholders_report.txt` |
| `--verbose`, `-v` | Enable verbose output | False |
| `--no-json` | Skip JSON output | False |
| `--no-report` | Skip readable report | False |
| `--summary-only` | Output only summary statistics (no individual matches) | False |
| `--max-file-size MB` | Maximum file size to scan in MB | 5 |
| `--include-hidden` | Include hidden files in scan | False |

## Pattern Types Detected

### Code Markers (Severity-based)
| Pattern | Severity | Description |
|---------|----------|-------------|
| `TODO` | LOW | Tasks to be completed |
| `FIXME` | HIGH | Known bugs requiring fixes |
| `HACK` | MEDIUM | Temporary workarounds |
| `XXX` | MEDIUM | Warnings/alerts |
| `BUG` | HIGH | Known bugs |
| `OPTIMIZE` | LOW | Performance improvements needed |
| `REVIEW` | LOW | Code needing review |
| `TEMP`/`TEMPORARY` | MEDIUM | Temporary code |
| `WORKAROUND` | MEDIUM | Workarounds in place |
| `DEPRECATED` | MEDIUM | Deprecated functionality |
| `STUB` | MEDIUM | Stub implementations |
| `UNIMPLEMENTED` | HIGH | Missing implementations |
| `INCOMPLETE` | MEDIUM | Incomplete code |
| `PLACEHOLDER` | MEDIUM | Explicit placeholders |

### Template Markers
| Pattern | Example |
|---------|---------|
| `mustache` | `{{variable}}` |
| `erb` | `<%= code %>` |
| `jinja` | `{% if condition %}` |
| `env_var` | `${API_KEY}` |
| `shell_var` | `$HOME` |

### Security Issues
| Pattern | Severity | Description |
|---------|----------|-------------|
| `hardcoded_secret` | CRITICAL | Hardcoded passwords/tokens |
| `debug_mode` | HIGH | Debug mode enabled |
| `localhost` | MEDIUM | Localhost references |
| `placeholder_text` | HIGH | Placeholder credentials |
| `dummy` | MEDIUM | Dummy/mock data |

## Output Formats

### JSON Output Structure

```json
{
  "scan_date": "2026-03-22T11:06:00",
  "total_placeholders": 1234,
  "by_type": {
    "TODO": 456,
    "FIXME": 78,
    "hardcoded_secret": 5
  },
  "by_severity": {
    "CRITICAL": 5,
    "HIGH": 89,
    "MEDIUM": 234,
    "LOW": 906
  },
  "by_file": {
    "src/main.rs": [
      {"line": 42, "type": "TODO", "severity": "LOW", "text": "..."}
    ]
  },
  "placeholders": [
    {
      "file": "src/main.rs",
      "line": 42,
      "column": 5,
      "type": "TODO",
      "severity": "LOW",
      "text": "    // TODO: implement error handling",
      "context_before": "    let result = risky_operation();",
      "context_after": "    match result {",
      "matched_text": "TODO"
    }
  ]
}
```

### Human-Readable Report

```
================================================================================
PLACEHOLDER SCAN REPORT
Generated: 2026-03-22 11:06:00
================================================================================

SUMMARY
----------------------------------------
Total placeholders found: 1234

By Severity:
  CRITICAL: 5
  HIGH: 89
  MEDIUM: 234
  LOW: 906

By Type:
  TODO: 456
  FIXME: 78
  NOTE: 234
  ...

================================================================================
DETAILED FINDINGS BY FILE
================================================================================

File: src/main.rs
  Placeholders: 12
----------------------------------------
  🔴 Line 42: [hardcoded_secret] (CRITICAL)
      password = "super_secret_123"
  🟠 Line 89: [FIXME] (HIGH)
      // FIXME: handle edge case
  🟢 Line 156: [TODO] (LOW)
      // TODO: add unit tests
```

## CI/CD Integration

### GitHub Actions

```yaml
name: Placeholder Scan
on: [push, pull_request]
jobs:
  scan:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-python@v5
        with:
          python-version: '3.x'
      - name: Run placeholder scan
        run: |
          python3 scan_placeholders.py --path . \
            --output placeholders.json \
            --report placeholder-report.txt
      - name: Upload results
        uses: actions/upload-artifact@v4
        with:
          name: placeholder-scan-results
          path: |
            placeholders.json
            placeholder-report.txt
      - name: Check for critical issues
        run: |
          critical=$(jq '.by_severity.CRITICAL // 0' placeholders.json)
          if [ "$critical" -gt 0 ]; then
            echo "::error::Found $critical CRITICAL placeholder issues!"
            exit 1
          fi
```

### Pre-commit Hook

```yaml
# .pre-commit-config.yaml
repos:
  - repo: local
    hooks:
      - id: placeholder-scan
        name: Scan for placeholders
        entry: python3 scan_placeholders.py --path . --no-report --summary-only
        language: system
        pass_filenames: false
```

## Auto-Create Issues from Placeholders

```bash
#!/bin/bash
# create-issues-from-placeholders.sh

# Read JSON and create GitHub issues for CRITICAL/HIGH severity
jq -c '.placeholders[] | select(.severity == "CRITICAL" or .severity == "HIGH")' \
  placeholders.json | while read -r entry; do
  
  file=$(echo "$entry" | jq -r '.file')
  line=$(echo "$entry" | jq -r '.line')
  type=$(echo "$entry" | jq -r '.type')
  severity=$(echo "$entry" | jq -r '.severity')
  text=$(echo "$entry" | jq -r '.text' | head -c 100)
  
  gh issue create \
    --title "[$severity] $type in $file:$line" \
    --body "Found at \`$file:$line\`:\n\`\`\`\n$text\n\`\`\`\n\nSeverity: $severity\nType: $type" \
    --label "bug,$(echo $severity | tr '[:upper:]' '[:lower:]')"
done
```

## Performance Considerations

- **Large repos**: Use `--summary-only` to reduce output size
- **Binary files**: Automatically detected and skipped via null-byte check
- **Large files**: Skipped if超过 `--max-file-size` (default 5MB)
- **Directories**: `.git`, `node_modules`, `target`, `build` auto-skipped
- **Hidden files**: Skipped by default (use `--include-hidden` to scan)

## Extending Patterns

Add custom patterns by modifying the `PATTERNS` dictionary in `scan_placeholders.py`:

```python
PATTERNS = {
    # Add your custom pattern
    'CUSTOM_MARKER': re.compile(r'\bCUSTOM\b', re.IGNORECASE),
    # ...
}
```

Also add severity classification:

```python
SEVERITY = {
    'CUSTOM_MARKER': 'MEDIUM',
    # ...
}
```

## Troubleshooting

### "File too large" errors
```bash
# Increase max file size
python3 scan_placeholders.py --path . --max-file-size 50
```

### Scan takes too long
```bash
# Use summary-only mode
python3 scan_placeholders.py --path . --summary-only

# Or scan specific directories
python3 scan_placeholders.py --path ./src --summary-only
```

### Too many false positives
- Review patterns in `PATTERNS` dictionary
- Add more directories to `SKIP_DIRS`
- Add file extensions to `SKIP_EXTENSIONS`

## License

This tool is part of the atlas-sphere/x3-chain project.