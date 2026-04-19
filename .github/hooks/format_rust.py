#!/usr/bin/env python3
"""PostToolUse hook: run `rustfmt` on any .rs files just edited.

Reads the VS Code agent hook JSON envelope from stdin and prints a JSON
response on stdout. Exits 0 on success. Never blocks agent execution --
formatting failures are reported as a non-fatal systemMessage so they
surface to the user but do not interrupt the session.
"""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]


def extract_paths(tool_input: dict) -> list[str]:
    """Gather every file path the tool claims to have touched.

    Covers the common VS Code / Copilot agent tool shapes:
      * { filePath }                 -- replace_string_in_file, create_file
      * { path }                     -- some file tools
      * { files: [...] }             -- editFiles, createFile (multi)
      * { replacements: [{filePath}] } -- multi_replace_string_in_file
    """
    paths: list[str] = []
    for key in ("filePath", "path", "file"):
        v = tool_input.get(key)
        if isinstance(v, str) and v:
            paths.append(v)
    files = tool_input.get("files")
    if isinstance(files, list):
        paths.extend(f for f in files if isinstance(f, str))
    reps = tool_input.get("replacements")
    if isinstance(reps, list):
        for r in reps:
            if isinstance(r, dict):
                fp = r.get("filePath")
                if isinstance(fp, str):
                    paths.append(fp)
    return paths


def resolve_rust_files(paths: list[str]) -> list[Path]:
    out: list[Path] = []
    for p in paths:
        if not p.endswith(".rs"):
            continue
        path = Path(p)
        if not path.is_absolute():
            path = (REPO_ROOT / path).resolve()
        if path.is_file():
            out.append(path)
    # Deduplicate while preserving order.
    seen: set[Path] = set()
    unique: list[Path] = []
    for f in out:
        if f not in seen:
            seen.add(f)
            unique.append(f)
    return unique


def main() -> int:
    try:
        data = json.load(sys.stdin)
    except Exception:
        # Not a valid hook envelope -- stay out of the way.
        print('{"continue":true}')
        return 0

    tool_input = data.get("tool_input") or {}
    rust_files = resolve_rust_files(extract_paths(tool_input))

    if not rust_files:
        print('{"continue":true}')
        return 0

    try:
        result = subprocess.run(
            ["rustfmt", "--edition", "2021", *map(str, rust_files)],
            check=False,
            capture_output=True,
            text=True,
            timeout=10,
        )
    except FileNotFoundError:
        # rustfmt not installed -- silent no-op so the hook stays friendly
        # on fresh clones.
        print('{"continue":true}')
        return 0
    except subprocess.TimeoutExpired:
        print(json.dumps({
            "continue": True,
            "systemMessage": "format_rust hook: rustfmt timed out (>10s)",
        }))
        return 0

    if result.returncode != 0:
        # Report but do not block.
        print(json.dumps({
            "continue": True,
            "systemMessage": (
                "format_rust hook: rustfmt exited "
                f"{result.returncode}. stderr: {result.stderr.strip()[:400]}"
            ),
        }))
        return 0

    print('{"continue":true}')
    return 0


if __name__ == "__main__":
    sys.exit(main())
