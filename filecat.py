#!/usr/bin/env python3
"""
filecat - Recursively search and display file contents with separator formatting.

Usage:
    python filecat.py [directory] [options]

Options:
    --max-lines N    Maximum lines to display per file (default: 200)
    --max-depth N    Maximum directory depth to recurse (default: unlimited)
"""

import os
import sys
import argparse

# Characters that indicate binary content (control chars excluding common whitespace)
_BINARY_THRESHOLD = 0.10  # If more than 10% of chars are binary, skip file


def is_binary_file(filepath, sample_size=8192):
    """Check if a file appears to be binary by reading a sample."""
    try:
        with open(filepath, "rb") as f:
            chunk = f.read(sample_size)
        if not chunk:
            return False
        # Count non-text bytes (control chars except \t, \n, \r)
        binary_count = 0
        for byte in chunk:
            if byte < 32 and byte not in (9, 10, 13):
                binary_count += 1
            elif byte > 126 and byte < 160:
                binary_count += 1
        return (binary_count / len(chunk)) > _BINARY_THRESHOLD
    except (IOError, OSError):
        return True


def is_hidden(path):
    """Check if a path component starts with a dot (hidden)."""
    parts = path.split(os.sep)
    return any(part.startswith(".") for part in parts)


def collect_files(root_dir, max_depth=None):
    """Recursively collect all files, skipping hidden dirs/files."""
    file_list = []
    root_dir = os.path.abspath(root_dir)

    for dirpath, dirnames, filenames in os.walk(root_dir):
        # Calculate current depth
        rel_path = os.path.relpath(dirpath, root_dir)
        if rel_path == ".":
            depth = 0
        else:
            depth = len(rel_path.split(os.sep))

        # Check depth limit
        if max_depth is not None and depth > max_depth:
            dirnames.clear()
            continue

        # Remove hidden directories from traversal (in-place modification)
        dirnames[:] = [d for d in dirnames if not d.startswith(".")]

        # Sort directories for consistent ordering
        dirnames.sort()

        # Collect non-hidden files
        for fname in sorted(filenames):
            if fname.startswith("."):
                continue
            full_path = os.path.join(dirpath, fname)
            rel_file_path = os.path.relpath(full_path, root_dir)
            file_list.append((rel_file_path, full_path))

    return file_list


def format_output(file_entries, max_lines):
    """Format and print all file entries with separators."""
    separator = "=" * 72
    lines = []

    lines.append(separator)
    lines.append("  FILE LISTING")
    lines.append(f"  Total files: {len(file_entries)}")
    lines.append(separator)

    for rel_path, full_path in file_entries:
        lines.append("")
        lines.append(separator)
        lines.append(f"  FILE: {rel_path}")
        lines.append(separator)

        # Check if binary
        if is_binary_file(full_path):
            lines.append("  [Binary file - skipped]")
            continue

        # Read text content
        try:
            with open(full_path, "r", encoding="utf-8", errors="replace") as f:
                content_lines = f.readlines()
        except (IOError, OSError) as e:
            lines.append(f"  [Error reading file: {e}]")
            continue

        # Output content (truncate if too long)
        if len(content_lines) > max_lines:
            for i, line in enumerate(content_lines[:max_lines]):
                lines.append(line.rstrip("\n"))
            truncated = len(content_lines) - max_lines
            lines.append(f"  ... [{truncated} more lines truncated] ...")
        else:
            for line in content_lines:
                lines.append(line.rstrip("\n"))

    lines.append("")
    lines.append(separator)
    lines.append("  END OF FILE LISTING")
    lines.append(separator)

    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(
        description="Recursively display file contents with separator formatting."
    )
    parser.add_argument(
        "directory",
        nargs="?",
        default=".",
        help="Root directory to search (default: current directory)",
    )
    parser.add_argument(
        "--max-lines",
        type=int,
        default=200,
        help="Maximum lines to display per file (default: 200)",
    )
    parser.add_argument(
        "--max-depth",
        type=int,
        default=None,
        help="Maximum directory depth (default: unlimited)",
    )

    args = parser.parse_args()

    if not os.path.isdir(args.directory):
        print(f"Error: '{args.directory}' is not a valid directory.", file=sys.stderr)
        sys.exit(1)

    # Collect files
    file_entries = collect_files(args.directory, args.max_depth)

    # Format and print
    output = format_output(file_entries, args.max_lines)
    print(output)

    return 0


if __name__ == "__main__":
    sys.exit(main())
