#!/usr/bin/env python3
"""
filetree - 재귀적 파일 검색 및 내용 출력 CLI 도구

현재 디렉토리와 하위 디렉토리의 파일을 재귀적으로 검색하고,
파일 목록과 내용을 구분선 포맷으로 출력합니다.

사용법:
    python filetree.py [디렉토리경로]
"""

import os
import sys

# 설정
MAX_LINES = 50
SEPARATOR = "=" * 70
SUB_SEPARATOR = "-" * 70

# 건너뛸 디렉토리
SKIP_DIRS = {
    ".git", ".svn", ".hg", "__pycache__", "node_modules",
    ".tox", ".mypy_cache", ".pytest_cache", ".idea", ".vscode",
    "target", "build", "dist", ".next", ".cache",
}

# 바이너리 확장자
BINARY_EXTENSIONS = {
    ".pyc", ".pyo", ".so", ".o", ".a", ".lib", ".dll", ".dylib",
    ".exe", ".bin", ".obj", ".elf", ".woff", ".woff2", ".ttf",
    ".eot", ".ico", ".png", ".jpg", ".jpeg", ".gif", ".bmp",
    ".tiff", ".webp", ".svg", ".mp3", ".mp4", ".avi", ".mov",
    ".wmv", ".flv", ".wav", ".ogg", ".flac", ".zip", ".tar",
    ".gz", ".bz2", ".xz", ".7z", ".rar", ".jar", ".war",
    ".pdf", ".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx",
    ".class", ".dex", ".apk", ".ipa", ".db", ".sqlite", ".mdb",
    ".iso", ".dmg", ".img", ".tmp", ".swp", ".swo", ".lock",
    ".wasm", ".dat", ".pak",
}

# 바이너리 판정을 위한 null byte 체크 바이트 수
BINARY_CHECK_BYTES = 8192


def is_binary(filepath):
    """파일이 바이너리인지 판정한다."""
    _, ext = os.path.splitext(filepath)
    if ext.lower() in BINARY_EXTENSIONS:
        return True

    try:
        with open(filepath, "rb") as f:
            chunk = f.read(BINARY_CHECK_BYTES)
        if b"\x00" in chunk:
            return True
    except (IOError, OSError):
        return True

    return False


def should_skip_dir(dirname):
    """건너뛸 디렉토리인지 판정한다."""
    if dirname.startswith("."):
        return True
    if dirname in SKIP_DIRS:
        return True
    return False


def should_skip_file(filename):
    """건너뛸 파일인지 판정한다."""
    if filename.startswith("."):
        return True
    return False


def collect_files(root_dir):
    """디렉토리 트리를 재귀적으로 순회하며 파일 목록을 수집한다."""
    files = []

    for dirpath, dirnames, filenames in os.walk(root_dir):
        # 건너뛸 디렉토리 제외 (in-place 수정)
        dirnames[:] = sorted(
            [d for d in dirnames if not should_skip_dir(d)]
        )

        for filename in sorted(filenames):
            if should_skip_file(filename):
                continue

            filepath = os.path.join(dirpath, filename)

            if not os.path.isfile(filepath):
                continue

            if is_binary(filepath):
                continue

            files.append(filepath)

    return files


def read_file_lines(filepath):
    """파일의 모든 줄을 읽어 반환한다."""
    try:
        with open(filepath, "r", encoding="utf-8", errors="replace") as f:
            return f.readlines()
    except (IOError, OSError) as e:
        return [f"<읽기 오류: {e}>\n"]


def format_size(size_bytes):
    """파일 크기를 읽기 쉬운 형식으로 포맷한다."""
    if size_bytes < 1024:
        return f"{size_bytes} B"
    elif size_bytes < 1024 * 1024:
        return f"{size_bytes / 1024:.1f} KB"
    else:
        return f"{size_bytes / (1024 * 1024):.1f} MB"


def print_output(files, root_dir):
    """파일 목록과 내용을 구분선 포맷으로 출력한다."""
    # 헤더
    print(SEPARATOR)
    print(f"  파일 트리 검색 결과: {os.path.abspath(root_dir)}")
    print(f"  검색된 파일 수: {len(files)}")
    print(SEPARATOR)
    print()

    for filepath in files:
        relpath = os.path.relpath(filepath, root_dir)
        file_size = os.path.getsize(filepath)
        file_lines = read_file_lines(filepath)
        total_lines = len(file_lines)

        # 파일 헤더
        print(SUB_SEPARATOR)
        print(f"  파일: {relpath}  ({format_size(file_size)})")
        print(f"  줄 수: {total_lines}")
        print(SUB_SEPARATOR)

        # 파일 내용 (최대 MAX_LINES줄)
        display_lines = file_lines[:MAX_LINES]
        for i, line in enumerate(display_lines, 1):
            print(f"  {i:>4} | {line.rstrip()}")

        # 잘림 메시지
        if total_lines > MAX_LINES:
            print(f"  ... (잘림: 총 {total_lines}줄)")

        print()

    # 푸터
    print(SEPARATOR)
    print(f"  총 파일 수: {len(files)}")
    print(SEPARATOR)


def main():
    root_dir = sys.argv[1] if len(sys.argv) > 1 else "."
    root_dir = os.path.abspath(root_dir)

    if not os.path.isdir(root_dir):
        print(
            f"오류: '{root_dir}' 디렉토리를 찾을 수 없습니다.",
            file=sys.stderr,
        )
        sys.exit(1)

    files = collect_files(root_dir)
    print_output(files, root_dir)


if __name__ == "__main__":
    main()
