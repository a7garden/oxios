#!/usr/bin/env python3
"""
fileviewer.py - 재귀적 파일 검색 및 내용 출력 CLI 도구

현재 디렉토리와 하위 디렉토리의 파일을 재귀적으로 검색하고,
파일 목록과 내용을 구분선 포맷으로 출력합니다.
바이너리 파일은 감지하여 건너뜁니다.

사용법:
    python fileviewer.py [디렉토리경로]

옵션:
    디렉토리경로  - 검색을 시작할 디렉토리 (기본값: 현재 디렉토리)
"""

import os
import sys

# 기본 설정
DEFAULT_MAX_LINES = 50       # 파일당 최대 출력 라인 수
SEPARATOR_WIDTH = 60         # 구분선 너비
SEPARATOR_CHAR = "="         # 구분선 문자
BINARY_NULL_BYTE_LIMIT = 8000  # 바이너리 감지를 위해 검사할 최대 바이트 수


def is_binary(file_path):
    """
    파일이 바이너리인지 감지합니다.
    파일의 앞부분을 읽어서 널 바이트(\x00)가 포함되어 있으면 바이너리로 판단합니다.
    """
    try:
        with open(file_path, "rb") as f:
            chunk = f.read(BINARY_NULL_BYTE_LIMIT)
        return b"\x00" in chunk
    except (IOError, OSError):
        return True  # 읽을 수 없는 파일도 바이너리로 취급


def is_hidden(path):
    """
    경로의 일부가 점(.)으로 시작하는지 확인하여 숨겨진 파일/디렉토리인지 판단합니다.
    """
    parts = path.replace("\\", "/").split("/")
    return any(part.startswith(".") for part in parts)


def print_separator(char=SEPARATOR_CHAR, width=SEPARATOR_WIDTH):
    """구분선을 출력합니다."""
    print(char * width)


def print_file_header(rel_path, file_size, is_bin):
    """
    파일 헤더를 출력합니다.
    파일 경로, 크기, 그리고 바이너리 여부를 표시합니다.
    """
    print_separator()
    print(f"파일: {rel_path}")
    print(f"크기: {file_size} bytes")
    if is_bin:
        print("바이너리 파일: 건너뜀")
    print_separator()


def print_file_content(file_path, max_lines=DEFAULT_MAX_LINES):
    """
    텍스트 파일의 내용을 출력합니다.
    max_lines를 초과하면 잘라내고 안내 메시지를 출력합니다.
    """
    try:
        with open(file_path, "r", encoding="utf-8", errors="replace") as f:
            line_count = 0
            for line in f:
                # 끝의 개행 제거 후 출력
                print(line, end="")
                line_count += 1
                if line_count >= max_lines:
                    remaining = "..."
                    print(f"\n... (나머지 내용 잘림, 최대 {max_lines}줄까지 출력) ...")
                    return
    except (IOError, OSError) as e:
        print(f"오류: 파일을 읽을 수 없습니다 - {e}")


def walk_directory(root_dir):
    """
    디렉토리를 재귀적으로 순회하며 파일 항목(FileEntry) 목록을 생성합니다.
    숨겨진 파일/디렉토리는 제외합니다.
    """
    file_entries = []
    for dirpath, dirnames, filenames in os.walk(root_dir):
        # 숨겨진 디렉토리 제외 (하위 탐색 방지)
        dirnames[:] = [
            d for d in dirnames
            if not d.startswith(".")
        ]
        for filename in filenames:
            # 숨겨진 파일 제외
            if filename.startswith("."):
                continue
            full_path = os.path.join(dirpath, filename)
            if is_hidden(os.path.relpath(full_path, root_dir)):
                continue
            try:
                file_size = os.path.getsize(full_path)
            except OSError:
                file_size = 0
            rel_path = os.path.relpath(full_path, root_dir)
            file_entries.append((rel_path, full_path, file_size))
    # 경로순 정렬
    file_entries.sort(key=lambda x: x[0])
    return file_entries


def main():
    """메인 진입점"""
    # 시작 디렉토리 결정
    if len(sys.argv) > 1:
        target_dir = sys.argv[1]
    else:
        target_dir = "."

    # 디렉토리 존재 확인
    if not os.path.isdir(target_dir):
        print(f"오류: '{target_dir}' 디렉토리를 찾을 수 없습니다.", file=sys.stderr)
        sys.exit(1)

    # 디렉토리를 절대경로로 변환
    target_dir = os.path.abspath(target_dir)

    print(f"📁 검색 디렉토리: {target_dir}")
    print()

    # 파일 목록 수집
    file_entries = walk_directory(target_dir)

    if not file_entries:
        print("검색된 파일이 없습니다.")
        return

    # 파일 목록 요약 출력
    print(f"총 {len(file_entries)}개의 파일을 찾았습니다.")
    print()

    # 각 파일 처리 및 출력
    for rel_path, full_path, file_size in file_entries:
        binary = is_binary(full_path)
        print_file_header(rel_path, file_size, binary)
        if not binary:
            print()
            print_file_content(full_path)
        print()  # 파일 간 빈 줄

    # 최종 구분선
    print_separator()
    print(f"완료: 총 {len(file_entries)}개 파일 처리됨")
    print_separator()


if __name__ == "__main__":
    main()
