#!/usr/bin/env python3
"""
Scan a repository for code placeholders (TODO, FIXME, template markers, etc.)
and generate JSON + human-readable reports.

Usage:
    python3 scan_placeholders.py --path /path/to/repo -o placeholders.json
    python3 scan_placeholders.py --url https://github.com/user/repo.git -o placeholders.json
"""

import os
import re
import sys
import json
import argparse
import subprocess
import tempfile
from datetime import datetime
from pathlib import Path
from typing import List, Dict, Set, Optional
from dataclasses import dataclass, asdict

# ============================================================================
# CONFIGURATION
# ============================================================================

# Maximum file size to scan (5MB) to avoid performance issues with large files
MAX_FILE_SIZE = 5 * 1024 * 1024

# Directories to skip entirely
SKIP_DIRS = {
    '.git', 'node_modules', 'target', 'build', 'dist', '__pycache__',
    '.pytest_cache', '.mypy_cache', '.tox', '.eggs', '*.egg-info',
    'vendor', 'third_party', 'venv', '.venv', 'env', '.env',
    'coverage', '.coverage', 'htmlcov', '.idea', '.vscode',
    'target_corrupt_20260319', 'target-chat', 'target-x3-turbine',
    'logs', 'data', 'media', 'cryptologos',
}

# File extensions to skip (binary/generated files)
SKIP_EXTENSIONS = {
    # Images
    '.png', '.jpg', '.jpeg', '.gif', '.bmp', '.ico', '.svg', '.webp',
    # Executables/Compiled
    '.exe', '.dll', '.so', '.dylib', '.o', '.obj', '.a', '.lib',
    # Archives
    '.zip', '.tar', '.gz', '.bz2', '.7z', '.rar', '.xz',
    # Media
    '.mp3', '.mp4', '.avi', '.mov', '.wav', '.flac', '.ogg',
    # Documents (binary)
    '.pdf', '.doc', '.docx', '.xls', '.xlsx', '.ppt', '.pptx',
    # Fonts
    '.ttf', '.otf', '.woff', '.woff2', '.eot',
    # Compiled WASM
    '.wasm',
    # Minified JS (usually too noisy)
    '.min.js', '.min.css',
    # Lock files (binary or huge)
    '.lock',
    # Database
    '.sqlite', '.db', '.mdb',
    # Other binary
    '.pyc', '.pyo', '.class', '.beam',
    # Generated HTML reports
    '.html',
}

# Patterns to search for - organized by category
PATTERNS = {
    # Standard code markers
    'TODO': re.compile(r'\bTODO\b', re.IGNORECASE),
    'FIXME': re.compile(r'\bFIXME\b', re.IGNORECASE),
    'HACK': re.compile(r'\bHACK\b', re.IGNORECASE),
    'XXX': re.compile(r'\bXXX\b', re.IGNORECASE),
    'BUG': re.compile(r'\bBUG\b', re.IGNORECASE),
    'OPTIMIZE': re.compile(r'\bOPTIMIZE\b', re.IGNORECASE),
    'REVIEW': re.compile(r'\bREVIEW\b', re.IGNORECASE),
    'TEMP': re.compile(r'\bTEMP(?:ORARY)?\b', re.IGNORECASE),
    'WORKAROUND': re.compile(r'\bWORKAROUND\b', re.IGNORECASE),
    'DEPRECATED': re.compile(r'\bDEPRECATED\b', re.IGNORECASE),
    'CHANGED': re.compile(r'\bCHANGED\b', re.IGNORECASE),
    'NOTE': re.compile(r'\bNOTE\b', re.IGNORECASE),
    'WARNING': re.compile(r'\bWARNING\b', re.IGNORECASE),
    'IMPORTANT': re.compile(r'\bIMPORTANT\b', re.IGNORECASE),
    'ATTENTION': re.compile(r'\bATTENTION\b', re.IGNORECASE),
    'PLACEHOLDER': re.compile(r'\bPLACEHOLDER\b', re.IGNORECASE),
    'STUB': re.compile(r'\bSTUB\b', re.IGNORECASE),
    'UNIMPLEMENTED': re.compile(r'\bUNIMPLEMENTED\b', re.IGNORECASE),
    'INCOMPLETE': re.compile(r'\bINCOMPLETE\b', re.IGNORECASE),

    # Template markers
    'mustache': re.compile(r'\{\{[^}]+\}\}'),
    'erb': re.compile(r'<%[^%]+%>'),
    'jinja': re.compile(r'\{%[^%]+%\}'),
    'env_var': re.compile(r'\$\{[A-Z_][A-Z0-9_]*\}'),
    'shell_var': re.compile(r'\$[A-Z_][A-Z0-9_]*\b'),

    # Common placeholder text
    'lorem': re.compile(r'\b(?:lorem|ipsum)\b', re.IGNORECASE),
    'example': re.compile(r'\b(?:example\.com|test\.com|foo\.bar)\b', re.IGNORECASE),
    'placeholder_text': re.compile(r'\b(?:your[_-]?api[_-]?key|your[_-]?token|your[_-]?password|replace[_-]?me|change[_-]?me)\b', re.IGNORECASE),
    'dummy': re.compile(r'\b(?:dummy|fake|mock|test)[\s_-]?(?:data|value|key|token)\b', re.IGNORECASE),

    # Potential security issues
    'hardcoded_secret': re.compile(r'(?:password|secret|token|api[_-]?key)\s*[=:]\s*["\'][^"\']{8,}["\']', re.IGNORECASE),
    'localhost': re.compile(r'\b(?:localhost|127\.0\.0\.1|0\.0\.0\.0)\b'),
    'debug_mode': re.compile(r'\b(?:DEBUG|debug)\s*[=:]\s*(?:true|True|TRUE|1)\b'),
}

# Severity levels for different placeholder types
SEVERITY = {
    'hardcoded_secret': 'CRITICAL',
    'debug_mode': 'HIGH',
    'localhost': 'MEDIUM',
    'TODO': 'LOW',
    'FIXME': 'HIGH',
    'HACK': 'MEDIUM',
    'XXX': 'MEDIUM',
    'BUG': 'HIGH',
    'OPTIMIZE': 'LOW',
    'REVIEW': 'LOW',
    'TEMP': 'MEDIUM',
    'WORKAROUND': 'MEDIUM',
    'DEPRECATED': 'MEDIUM',
    'STUB': 'MEDIUM',
    'UNIMPLEMENTED': 'HIGH',
    'INCOMPLETE': 'MEDIUM',
    'placeholder_text': 'HIGH',
    'dummy': 'MEDIUM',
    'lorem': 'LOW',
    'example': 'LOW',
}

DEFAULT_SEVERITY = 'INFO'

# ============================================================================
# DATA CLASSES
# ============================================================================

@dataclass
class Placeholder:
    file: str
    line: int
    column: int
    type: str
    severity: str
    text: str
    context_before: str
    context_after: str
    matched_text: str

# ============================================================================
# UTILITY FUNCTIONS
# ============================================================================

def is_text_file(path: str, blocksize: int = 512) -> bool:
    """Check if a file is text by looking for null bytes."""
    try:
        with open(path, 'rb') as f:
            block = f.read(blocksize)
        return b'\0' not in block
    except Exception:
        return False

def should_skip_file(filepath: str, filename: str) -> bool:
    """Determine if a file should be skipped."""
    # Check extension
    _, ext = os.path.splitext(filename.lower())
    if ext in SKIP_EXTENSIONS:
        return True

    # Skip hidden files (except .github, .vscode configs we want to scan)
    if filename.startswith('.') and filename not in {'.gitignore', '.env.example', '.env.template'}:
        return True

    return False

def get_relative_path(filepath: str, base_dir: str) -> str:
    """Get the relative path from base directory."""
    try:
        return os.path.relpath(filepath, base_dir)
    except ValueError:
        return filepath

def scan_file(filepath: str, relpath: str, context_lines: int = 1) -> List[Placeholder]:
    """Scan a single file for placeholder patterns."""
    results = []

    # Skip if not a text file
    if not is_text_file(filepath):
        return results

    try:
        with open(filepath, 'r', encoding='utf-8', errors='ignore') as f:
            lines = f.readlines()
    except Exception as e:
        print(f"Warning: Could not read {relpath}: {e}", file=sys.stderr)
        return results

    for line_num, line in enumerate(lines, 1):
        for pattern_name, pattern in PATTERNS.items():
            for match in pattern.finditer(line):
                # Get context lines
                ctx_before = lines[line_num - 2].rstrip() if line_num >= 2 else ''
                ctx_after = lines[line_num].rstrip() if line_num < len(lines) else ''

                placeholder = Placeholder(
                    file=relpath,
                    line=line_num,
                    column=match.start() + 1,
                    type=pattern_name,
                    severity=SEVERITY.get(pattern_name, DEFAULT_SEVERITY),
                    text=line.rstrip(),
                    context_before=ctx_before,
                    context_after=ctx_after,
                    matched_text=match.group(0)
                )
                results.append(placeholder)

    return results

def scan_directory(base_dir: str, verbose: bool = False) -> List[Placeholder]:
    """Recursively scan a directory for placeholders."""
    all_results = []
    files_scanned = 0
    files_skipped = 0

    for root, dirs, files in os.walk(base_dir):
        # Filter out directories to skip
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS]

        for filename in files:
            filepath = os.path.join(root, filename)
            relpath = get_relative_path(filepath, base_dir)

            # Check if we should skip this file
            if should_skip_file(filepath, filename):
                files_skipped += 1
                continue

            # Check file size
            try:
                file_size = os.path.getsize(filepath)
                if file_size > MAX_FILE_SIZE:
                    if verbose:
                        print(f"Skipping large file ({file_size / 1024 / 1024:.1f}MB): {relpath}", file=sys.stderr)
                    files_skipped += 1
                    continue
                if file_size == 0:
                    continue
            except OSError:
                continue

            # Scan the file
            results = scan_file(filepath, relpath)
            all_results.extend(results)
            files_scanned += 1

            if verbose and files_scanned % 100 == 0:
                print(f"Scanned {files_scanned} files, found {len(all_results)} placeholders...", file=sys.stderr)

    if verbose:
        print(f"\nScan complete: {files_scanned} files scanned, {files_skipped} files skipped, {len(all_results)} placeholders found", file=sys.stderr)

    return all_results

def clone_repo(git_url: str) -> str:
    """Clone a Git repo to a temporary directory."""
    tmpdir = tempfile.mkdtemp(prefix="placeholder-scan-")
    print(f"Cloning {git_url} to {tmpdir}...", file=sys.stderr)

    cmd = ['git', 'clone', '--depth', '1', git_url, tmpdir]
    try:
        subprocess.check_call(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        return tmpdir
    except subprocess.CalledProcessError as e:
        print(f"Error cloning {git_url}: {e}", file=sys.stderr)
        sys.exit(1)

# ============================================================================
# OUTPUT FORMATTING
# ============================================================================

def generate_json_report(results: List[Placeholder], output_path: Optional[str] = None, summary_only: bool = False) -> str:
    """Generate JSON report from scan results."""
    report = {
        'scan_date': datetime.now().isoformat(),
        'total_placeholders': len(results),
        'by_type': {},
        'by_severity': {},
        'by_file': {},
    }

    # Aggregate statistics
    for p in results:
        # By type
        report['by_type'][p.type] = report['by_type'].get(p.type, 0) + 1
        # By severity
        report['by_severity'][p.severity] = report['by_severity'].get(p.severity, 0) + 1
        # By file (summary only - just count, not details)
        if summary_only:
            if p.file not in report['by_file']:
                report['by_file'][p.file] = {'count': 0, 'types': set()}
            report['by_file'][p.file]['count'] += 1
            report['by_file'][p.file]['types'].add(p.type)
        else:
            if p.file not in report['by_file']:
                report['by_file'][p.file] = []
            report['by_file'][p.file].append({
                'line': p.line,
                'type': p.type,
                'severity': p.severity,
                'text': p.text
            })

    # Convert sets to lists for JSON serialization in summary mode
    if summary_only:
        for file_info in report['by_file'].values():
            file_info['types'] = sorted(file_info['types'])
        # Don't include individual placeholders in summary mode
        report['mode'] = 'summary'
    else:
        report['placeholders'] = [asdict(p) for p in results]
        report['mode'] = 'full'

    json_str = json.dumps(report, indent=2, ensure_ascii=False)

    if output_path:
        with open(output_path, 'w', encoding='utf-8') as f:
            f.write(json_str)
        print(f"JSON report written to: {output_path}", file=sys.stderr)

    return json_str

def generate_readable_report(results: List[Placeholder], output_path: Optional[str] = None) -> str:
    """Generate human-readable report from scan results."""
    lines = []
    lines.append("=" * 80)
    lines.append("PLACEHOLDER SCAN REPORT")
    lines.append(f"Generated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    lines.append("=" * 80)
    lines.append("")

    # Summary statistics
    lines.append("SUMMARY")
    lines.append("-" * 40)
    lines.append(f"Total placeholders found: {len(results)}")
    lines.append("")

    # By severity
    severity_counts = {}
    for p in results:
        severity_counts[p.severity] = severity_counts.get(p.severity, 0) + 1

    lines.append("By Severity:")
    for sev in ['CRITICAL', 'HIGH', 'MEDIUM', 'LOW', 'INFO']:
        count = severity_counts.get(sev, 0)
        if count > 0:
            lines.append(f"  {sev}: {count}")
    lines.append("")

    # By type
    type_counts = {}
    for p in results:
        type_counts[p.type] = type_counts.get(p.type, 0) + 1

    lines.append("By Type:")
    for typ, count in sorted(type_counts.items(), key=lambda x: -x[1]):
        lines.append(f"  {typ}: {count}")
    lines.append("")

    # Group by file
    lines.append("=" * 80)
    lines.append("DETAILED FINDINGS BY FILE")
    lines.append("=" * 80)

    file_groups = {}
    for p in results:
        if p.file not in file_groups:
            file_groups[p.file] = []
        file_groups[p.file].append(p)

    for filepath in sorted(file_groups.keys()):
        placeholders = file_groups[filepath]
        lines.append("")
        lines.append(f"File: {filepath}")
        lines.append(f"  Placeholders: {len(placeholders)}")
        lines.append("-" * 40)

        # Sort by line number
        placeholders.sort(key=lambda p: p.line)

        for p in placeholders:
            severity_marker = {
                'CRITICAL': '🔴',
                'HIGH': '🟠',
                'MEDIUM': '🟡',
                'LOW': '🟢',
                'INFO': '⚪'
            }.get(p.severity, '⚪')

            lines.append(f"  {severity_marker} Line {p.line}: [{p.type}] ({p.severity})")
            lines.append(f"      {p.text.strip()}")

    lines.append("")
    lines.append("=" * 80)
    lines.append("END OF REPORT")
    lines.append("=" * 80)

    report = '\n'.join(lines)

    if output_path:
        with open(output_path, 'w', encoding='utf-8') as f:
            f.write(report)
        print(f"Readable report written to: {output_path}", file=sys.stderr)

    return report

# ============================================================================
# MAIN
# ============================================================================

def main():
    parser = argparse.ArgumentParser(
        description="Scan repository for code placeholders (TODO, FIXME, template markers, etc.)",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Scan local directory
  python3 scan_placeholders.py --path /path/to/repo

  # Clone and scan from GitHub
  python3 scan_placeholders.py --url https://github.com/user/repo.git

  # Scan with custom output
  python3 scan_placeholders.py --path . -o results.json --report results.txt

  # Verbose mode
  python3 scan_placeholders.py --path . -v
        """
    )
    parser.add_argument('--path', help="Path to local repository", default=None)
    parser.add_argument('--url', help="Git clone URL (overrides path)", default=None)
    parser.add_argument('--output', '-o', help="Output JSON file path", default='placeholders.json')
    parser.add_argument('--report', '-r', help="Output readable report file path", default='placeholders_report.txt')
    parser.add_argument('--verbose', '-v', action='store_true', help="Enable verbose output")
    parser.add_argument('--no-json', action='store_true', help="Skip JSON output")
    parser.add_argument('--no-report', action='store_true', help="Skip readable report")
    parser.add_argument('--summary-only', action='store_true', help="Output only summary statistics (no individual placeholder details)")

    args = parser.parse_args()

    # Determine directory to scan
    if args.url:
        repo_dir = clone_repo(args.url)
        cleanup_needed = True
    else:
        repo_dir = args.path or os.getcwd()
        cleanup_needed = False

    if not os.path.isdir(repo_dir):
        print(f"Error: directory not found: {repo_dir}", file=sys.stderr)
        sys.exit(1)

    print(f"Scanning repository: {repo_dir}", file=sys.stderr)
    print(f"Patterns to search: {len(PATTERNS)}", file=sys.stderr)
    print("", file=sys.stderr)

    # Run the scan
    results = scan_directory(repo_dir, verbose=args.verbose)

    # Generate outputs
    if not args.no_json:
        generate_json_report(results, args.output, summary_only=args.summary_only)

    if not args.no_report:
        if args.summary_only:
            # In summary-only mode, generate a condensed report
            report_lines = []
            report_lines.append("=" * 80)
            report_lines.append("PLACEHOLDER SCAN SUMMARY")
            report_lines.append(f"Generated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
            report_lines.append("=" * 80)
            report_lines.append("")
            report_lines.append(f"Total placeholders found: {len(results)}")
            report_lines.append("")
            
            # By severity
            severity_counts = {}
            for p in results:
                severity_counts[p.severity] = severity_counts.get(p.severity, 0) + 1
            report_lines.append("By Severity:")
            for sev in ['CRITICAL', 'HIGH', 'MEDIUM', 'LOW', 'INFO']:
                count = severity_counts.get(sev, 0)
                if count > 0:
                    report_lines.append(f"  {sev}: {count}")
            report_lines.append("")
            
            # By type
            type_counts = {}
            for p in results:
                type_counts[p.type] = type_counts.get(p.type, 0) + 1
            report_lines.append("By Type:")
            for typ, count in sorted(type_counts.items(), key=lambda x: -x[1]):
                report_lines.append(f"  {typ}: {count}")
            report_lines.append("")
            
            # Top files
            file_counts = {}
            for p in results:
                file_counts[p.file] = file_counts.get(p.file, 0) + 1
            report_lines.append("Top 20 Files:")
            for f, count in sorted(file_counts.items(), key=lambda x: -x[1])[:20]:
                report_lines.append(f"  {f}: {count} placeholders")
            
            report_lines.append("")
            report_lines.append("=" * 80)
            
            report = '\n'.join(report_lines)
            with open(args.report, 'w', encoding='utf-8') as f:
                f.write(report)
            print(f"Summary report written to: {args.report}", file=sys.stderr)
            print("\n" + report)
        else:
            report = generate_readable_report(results, args.report)
            # Also print summary to stdout
            print("\n" + report)

    # Cleanup if we cloned
    if cleanup_needed:
        import shutil
        shutil.rmtree(repo_dir)
        print(f"Cleaned up temporary directory: {repo_dir}", file=sys.stderr)

    # Return non-zero if critical/high severity issues found
    critical_high = sum(1 for p in results if p.severity in ('CRITICAL', 'HIGH'))
    if critical_high > 0:
        print(f"\n⚠️  Found {critical_high} CRITICAL/HIGH severity issues!", file=sys.stderr)
        return 1

    return 0

if __name__ == "__main__":
    sys.exit(main())