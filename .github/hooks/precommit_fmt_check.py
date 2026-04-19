#!/usr/bin/env python3
"""PreToolUse hook: block `git commit` if `cargo fmt --check` fails.

Only fires when the agent is about to run a terminal command that contains
`git commit` (or `git commit -m`, `git commit -F`, etc.). Runs
`cargo fmt --all -- --check` first; if that exits non-zero, denies the
tool invocation with a clear reason so the agent knows to run
`cargo fmt --all` before retrying.

Never blocks anything other than `git commit`. Never runs cargo if the
command isn't a commit (so normal shell commands stay fast).
"""
from __future__ import annotations

import json
import re
import shlex
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]

# Matches "git commit", "git  commit", "git -C foo commit", etc. Word-boundary
# anchored so `git commit-msg-hook` or similar wouldn't match.
_GIT_COMMIT_RE = re.compile(r"(^|[\s;&|])git(\s+-[^\s]+)*\s+commit(\s|$)")


def looks_like_git_commit(command: str) -> bool:
    return bool(_GIT_COMMIT_RE.search(command))


def extract_command(tool_input: dict) -> str | None:
    """Pull the shell command out of the tool_input regardless of key name."""
    for key in ("command", "cmd", "input", "script"):
        v = tool_input.get(key)
        if isinstance(v, str) and v:
            return v
    # Some tools pass an args array plus a command name.
    args = tool_input.get("args")
    if isinstance(args, list) and all(isinstance(a, str) for a in args):
        cmd_name = tool_input.get("command") or ""
        return " ".join([cmd_name, *(shlex.quote(a) for a in args)]).strip()
    return None


def is_terminal_tool(tool_name: str) -> bool:
    name = tool_name.lower()
    return (
        "terminal" in name
        or name in {"run_in_terminal", "runterminalcommand", "shell", "bash"}
    )


def main() -> int:
    try:
        data = json.load(sys.stdin)
    except Exception:
        print('{"continue":true}')
        return 0

    tool_name = data.get("tool_name") or ""
    tool_input = data.get("tool_input") or {}

    if not is_terminal_tool(tool_name):
        print('{"continue":true}')
        return 0

    command = extract_command(tool_input) or ""
    if not looks_like_git_commit(command):
        print('{"continue":true}')
        return 0

    # Run cargo fmt --check from the repo root.
    try:
        result = subprocess.run(
            ["cargo", "fmt", "--all", "--", "--check"],
            check=False,
            capture_output=True,
            text=True,
            timeout=25,
            cwd=str(REPO_ROOT),
        )
    except FileNotFoundError:
        # No cargo on PATH -- don't block.
        print('{"continue":true}')
        return 0
    except subprocess.TimeoutExpired:
        print(json.dumps({
            "continue": True,
            "systemMessage": "precommit_fmt_check hook: cargo fmt --check timed out (>25s); allowing commit.",
        }))
        return 0

    if result.returncode == 0:
        print('{"continue":true}')
        return 0

    # Format check failed -- deny this one tool call so the agent runs
    # `cargo fmt --all` before retrying the commit.
    reason = (
        "cargo fmt --check failed. Run `cargo fmt --all` and re-commit. "
        f"rustfmt output (truncated):\n{(result.stdout or result.stderr).strip()[:600]}"
    )
    print(json.dumps({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }
    }))
    return 0


if __name__ == "__main__":
    sys.exit(main())
