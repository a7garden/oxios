#!/usr/bin/env python3
"""
dumptree - Recursively dump directory files with separator-formatted output.

Usage:
    python dumptree.py [PATH] [OPTIONS]

Arguments:
    PATH          Root directory to scan (default: current directory)

Options:
    --max-lines N     Maximum lines to show per file (default: 100)
    --max-depth N     Maximum directory depth (default: unlimited)
    --no-truncate     Show full file contents without truncation
    -h, --help        Show this help message
"""

import os
import sys
import argparse

# ── Configuration ────────────────────────────────────────────────────────────

SEPARATOR = "════════════════════════════════════════════════════════════════"
THIN_SEP  = "──────────────────────────────────────────────────────────────────"

# Directories to always skip
SKIP_DIRS = {
    ".git", ".svn", ".hg", "node_modules", "__pycache__",
    ".tox", ".mypy_cache", ".pytest_cache", "target",
    ".idea", ".vscode", "venv", ".venv", "dist", "build",
    ".next", ".cache", ".gradle",
}

# File extensions that are typically binary
BINARY_EXTENSIONS = {
    ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".ico", ".webp", ".svg",
    ".mp3", ".mp4", ".avi", ".mov", ".mkv", ".flv", ".wav", ".ogg",
    ".zip", ".tar", ".gz", ".bz2", ".xz", ".7z", ".rar",
    ".pdf", ".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx",
    ".exe", ".dll", ".so", ".dylib", ".o", ".obj", ".a", ".lib",
    ".class", ".jar", ".war", ".pyc", ".pyo", ".wasm",
    ".db", ".sqlite", ".sqlite3",
    ".ttf", ".otf", ".woff", ".woff2", ".eot",
    ".lock",  # lock files are usually large generated files
}


def is_binary_file(filepath):
    """Detect binary files by extension and content sniffing."""
    _, ext = os.path.splitext(filepath)
    ext = ext.lower()

    if ext in BINARY_EXTENSIONS:
        return True

    # Content sniffing: read first 8192 bytes and check for null bytes
    try:
        with open(filepath, "rb") as f:
            chunk = f.read(8192)
        if b"\x00" in chunk:
            return True
    except (OSError, PermissionError):
        return True

    return False


def should_skip_dir(dirname):
    """Check if a directory should be skipped."""
    return dirname.startswith(".") or dirname in SKIP_DIRS


def collect_files(root_path, max_depth=None):
    """Recursively collect text files under root_path."""
    files = []
    root_path = os.path.normpath(root_path)

    for dirpath, dirnames, filenames in os.walk(root_path):
        # Compute current depth
        rel = os.path.relpath(dirpath, root_path)
        depth = 0 if rel == "." else rel.count(os.sep) + 1

        # Prune directories in-place so os.walk doesn't descend into them
        dirnames[:] = [
            d for d in sorted(dirnames)
            if not should_skip_dir(d)
        ]

        if max_depth is not None and depth > max_depth:
            dirnames.clear()
            continue

        for fname in sorted(filenames):
            # Skip hidden files
            if fname.startswith("."):
                continue

            fpath = os.path.join(dirpath, fname)

            # Skip directories and symlinks
            if not os.path.isfile(fpath) or os.path.islink(fpath):
                continue

            # Skip binary files
            if is_binary_file(fpath):
                continue

            files.append(fpath)

    return files


def format_file_output(filepath, max_lines=None):
    """Format a single file's output with separator and header."""
    lines = []
    rel_path = os.path.relpath(filepath)

    # Get file size
    try:
        size = os.path.getsize(filepath)
        size_str = _human_readable_size(size)
    except OSError:
        size_str = "unknown"

    lines.append(SEPARATOR)
    lines.append(f"  File: {rel_path}  ({size_str})")
    lines.append(SEPARATOR)

    try:
        with open(filepath, "r", encoding="utf-8", errors="replace") as f:
            content_lines = f.readlines()

        if max_lines is not None and len(content_lines) > max_lines:
            for i, line in enumerate(content_lines[:max_lines], start=1):
                lines.append(f"  {i:>4} │ {line.rstrip()}")
            lines.append(THIN_SEP)
            remaining = len(content_lines) - max_lines
            lines.append(f"  ... {remaining} more lines (truncated) ...")
        else:
            for i, line in enumerate(content_lines, start=1):
                lines.append(f"  {i:>4} │ {line.rstrip()}")

    except (OSError, PermissionError) as e:
        lines.append(f"  [Error reading file: {e}]")

    lines.append("")
    return "\n".join(lines)


def _human_readable_size(size_bytes):
    """Convert bytes to human-readable size string."""
    for unit in ("B", "KB", "MB", "GB"):
        if size_bytes < 1024:
            return f"{size_bytes:.0f} {unit}"
        size_bytes /= 1024
    return f"{size_bytes:.0f} TB"


def main():
    parser = argparse.ArgumentParser(
        prog="dumptree",
        description="Recursively dump directory files with separator-formatted output.",
    )
    parser.add_argument(
        "path",
        nargs="?",
        default=".",
        help="Root directory to scan (default: current directory)",
    )
    parser.add_argument(
        "--max-lines",
        type=int,
        default=100,
        help="Maximum lines to show per file (default: 100)",
    )
    parser.add_argument(
        "--max-depth",
        type=int,
        default=None,
        help="Maximum directory depth (default: unlimited)",
    )
    parser.add_argument(
        "--no-truncate",
        action="store_true",
        help="Show full file contents without truncation",
    )

    args = parser.parse_args()

    root = os.path.abspath(args.path)
    if not os.path.isdir(root):
        print(f"Error: '{args.path}' is not a directory.", file=sys.stderr)
        sys.exit(1)

    max_lines = None if args.no_truncate else args.max_lines

    # Collect files
    files = collect_files(root, max_depth=args.max_depth)

    if not files:
        print("No text files found.")
        sys.exit(0)

    # Summary header
    print(SEPARATOR)
    print(f"  Directory: {os.path.relpath(root) if root != os.getcwd() else '.'}")
    print(f"  Files found: {len(files)}")
    print(SEPARATOR)
    print()

    # Output each file
    for filepath in files:
        output = format_file_output(filepath, max_lines=max_lines)
        print(output)


if __name__ == "__main__":
    main()
