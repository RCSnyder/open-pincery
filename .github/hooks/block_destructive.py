#!/usr/bin/env python3
"""PreToolUse hook: refuse destructive commands the project forbids.

Implements the STOP rules from .github/copilot-instructions.md:
  * Never use `git reset --hard`
  * Never use `git push --force` / `-f`
  * Never use `git rebase` (interactive or otherwise)
  * Never use `--no-verify`
  * Never use `rm -rf` on sensitive paths
  * Never DROP/DELETE without WHERE on the production DB

Returns a PreToolUse permissionDecision:deny with a reason pointing at the
rule. Pass-through for anything that doesn't match.
"""
from __future__ import annotations

import json
import re
import sys


# Each rule: (compiled regex, human-readable reason).
# Regexes match against the normalised command string (single-spaced).
RULES: list[tuple[re.Pattern[str], str]] = [
    (
        re.compile(r"\bgit\s+(-[^\s]+\s+)*reset\s+(--hard|--merge\s+.*--hard|-[^\s]*h)", re.I),
        "`git reset --hard` is banned — destroys working-tree changes. Use `git revert` (non-destructive) or commit a WIP checkpoint first.",
    ),
    (
        re.compile(r"\bgit\s+(-[^\s]+\s+)*push\s+(.*\s)?(--force(?!-with-lease)|--force-with-lease|-f)(\s|$)", re.I),
        "`git push --force` / `-f` is banned — rewrites remote history. Use `git revert` + normal push.",
    ),
    (
        re.compile(r"\bgit\s+(-[^\s]+\s+)*rebase\b", re.I),
        "`git rebase` is banned — rewrites local history. Linear history is maintained via `git revert` instead.",
    ),
    (
        re.compile(r"--no-verify\b", re.I),
        "`--no-verify` is banned — it bypasses hooks/CI checks that exist for good reasons.",
    ),
    (
        re.compile(r"\brm\s+(-[a-zA-Z]*r[a-zA-Z]*f[a-zA-Z]*|-[a-zA-Z]*f[a-zA-Z]*r[a-zA-Z]*|-rf|-fr)\s+(/|~|\$HOME|\.$|\.\.|\*)", re.I),
        "`rm -rf` on a sensitive path (/, ~, $HOME, ., .., or a bare glob) is banned. Target a specific subdirectory instead.",
    ),
    (
        re.compile(r"\bDROP\s+(TABLE|SCHEMA|DATABASE)\b", re.I),
        "`DROP TABLE/SCHEMA/DATABASE` is banned at the tool level — use migrations or ask the user first.",
    ),
    (
        re.compile(r"\bDELETE\s+FROM\s+\w+\s*(;|$)", re.I),
        "`DELETE FROM <table>` without a WHERE clause is banned. Add a predicate or use TRUNCATE with explicit confirmation.",
    ),
    (
        re.compile(r"\bTRUNCATE\s+(TABLE\s+)?\w+\b", re.I),
        "`TRUNCATE` is banned via hook — ask the user before wiping a table.",
    ),
]


def is_terminal_tool(tool_name: str) -> bool:
    name = (tool_name or "").lower()
    return (
        "terminal" in name
        or name in {"run_in_terminal", "runterminalcommand", "shell", "bash"}
    )


def extract_command(tool_input: dict) -> str:
    for key in ("command", "cmd", "input", "script"):
        v = tool_input.get(key)
        if isinstance(v, str) and v:
            return v
    return ""


def main() -> int:
    try:
        data = json.load(sys.stdin)
    except Exception:
        print('{"continue":true}')
        return 0

    if not is_terminal_tool(data.get("tool_name") or ""):
        print('{"continue":true}')
        return 0

    command = extract_command(data.get("tool_input") or {})
    if not command:
        print('{"continue":true}')
        return 0

    # Normalise whitespace for matching, but preserve original for the reason.
    normalised = " ".join(command.split())

    for pattern, reason in RULES:
        if pattern.search(normalised):
            print(json.dumps({
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "deny",
                    "permissionDecisionReason": (
                        f"{reason} See .github/copilot-instructions.md "
                        "(Non-destructive history / STOP conditions)."
                    ),
                }
            }))
            return 0

    print('{"continue":true}')
    return 0


if __name__ == "__main__":
    sys.exit(main())
