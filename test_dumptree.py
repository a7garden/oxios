#!/usr/bin/env python3
"""Quick validation test for dumptree.py"""

import os
import sys
import tempfile
import shutil

# Add current dir to path
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from dumptree import (
    SEPARATOR,
    THIN_SEP,
    is_binary_file,
    should_skip_dir,
    collect_files,
    format_file_output,
    _human_readable_size,
)


def test_separator_format():
    """Separator should be the expected ═ format."""
    assert SEPARATOR.startswith("═"), f"Separator should start with ═, got: {SEPARATOR[0]}"
    assert len(SEPARATOR) > 30, f"Separator too short: {len(SEPARATOR)}"
    print("✓ Separator format OK")


def test_human_readable_size():
    assert _human_readable_size(0) == "0 B"
    assert _human_readable_size(512) == "512 B"
    assert _human_readable_size(1024) == "1 KB"
    assert _human_readable_size(1048576) == "1 MB"
    print("✓ Human-readable size OK")


def test_skip_dirs():
    assert should_skip_dir(".git") is True
    assert should_skip_dir(".hidden") is True
    assert should_skip_dir("node_modules") is True
    assert should_skip_dir("target") is True
    assert should_skip_dir("src") is False
    assert should_skip_dir("my-folder") is False
    print("✓ Skip directories OK")


def test_binary_detection():
    tmpdir = tempfile.mkdtemp()
    try:
        # Text file
        txt = os.path.join(tmpdir, "test.txt")
        with open(txt, "w") as f:
            f.write("Hello World\n")
        assert is_binary_file(txt) is False

        # Binary file (null bytes)
        binf = os.path.join(tmpdir, "test.bin")
        with open(binf, "wb") as f:
            f.write(b"Hello\x00World")
        assert is_binary_file(binf) is True

        # Known extension
        png = os.path.join(tmpdir, "test.png")
        with open(png, "w") as f:
            f.write("fake")
        assert is_binary_file(png) is True

        print("✓ Binary detection OK")
    finally:
        shutil.rmtree(tmpdir)


def test_collect_files():
    tmpdir = tempfile.mkdtemp()
    try:
        # Create structure
        os.makedirs(os.path.join(tmpdir, "sub"))
        with open(os.path.join(tmpdir, "a.txt"), "w") as f:
            f.write("aaa")
        with open(os.path.join(tmpdir, "sub", "b.py"), "w") as f:
            f.write("bbb")
        with open(os.path.join(tmpdir, ".hidden"), "w") as f:
            f.write("hidden")
        os.makedirs(os.path.join(tmpdir, ".secret"))
        with open(os.path.join(tmpdir, ".secret", "c.txt"), "w") as f:
            f.write("secret")

        files = collect_files(tmpdir)
        basenames = [os.path.basename(f) for f in files]
        assert "a.txt" in basenames, f"Missing a.txt in {basenames}"
        assert "b.py" in basenames, f"Missing b.py in {basenames}"
        assert ".hidden" not in basenames, "Hidden file should be excluded"
        assert "c.txt" not in basenames, "File in hidden dir should be excluded"
        print("✓ Collect files OK")
    finally:
        shutil.rmtree(tmpdir)


def test_format_output():
    tmpdir = tempfile.mkdtemp()
    try:
        fpath = os.path.join(tmpdir, "test.txt")
        with open(fpath, "w") as f:
            f.write("line1\nline2\nline3\n")

        output = format_file_output(fpath, max_lines=2)
        assert SEPARATOR in output, "Output should contain separator"
        assert "test.txt" in output, "Output should contain filename"
        assert "line1" in output, "Output should contain file content"
        assert "truncated" in output, "Output should indicate truncation"
        assert "1 more lines" in output, "Should show remaining count"
        print("✓ Format output OK")
    finally:
        shutil.rmtree(tmpdir)


def test_max_depth():
    tmpdir = tempfile.mkdtemp()
    try:
        os.makedirs(os.path.join(tmpdir, "a", "b", "c"))
        with open(os.path.join(tmpdir, "root.txt"), "w") as f:
            f.write("r")
        with open(os.path.join(tmpdir, "a", "a.txt"), "w") as f:
            f.write("a")
        with open(os.path.join(tmpdir, "a", "b", "b.txt"), "w") as f:
            f.write("b")
        with open(os.path.join(tmpdir, "a", "b", "c", "c.txt"), "w") as f:
            f.write("c")

        files_d0 = collect_files(tmpdir, max_depth=0)
        basenames_d0 = [os.path.basename(f) for f in files_d0]
        assert "root.txt" in basenames_d0
        assert "a.txt" not in basenames_d0

        files_d1 = collect_files(tmpdir, max_depth=1)
        basenames_d1 = [os.path.basename(f) for f in files_d1]
        assert "root.txt" in basenames_d1
        assert "a.txt" in basenames_d1
        assert "b.txt" not in basenames_d1

        print("✓ Max depth OK")
    finally:
        shutil.rmtree(tmpdir)


if __name__ == "__main__":
    test_separator_format()
    test_human_readable_size()
    test_skip_dirs()
    test_binary_detection()
    test_collect_files()
    test_format_output()
    test_max_depth()
    print()
    print("All tests passed! ✓")
