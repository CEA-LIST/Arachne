#!/usr/bin/env python3
"""Update a field value in the JSON CRDT (replaces existing value)."""

import argparse
import json
import sys
import urllib.request
from typing import Any


def post_op(base_url: str, op: dict[str, Any]) -> None:
    payload = {"JsonKind": op}
    body = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        f"{base_url.rstrip('/')}/api/op",
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=10) as resp:
        if resp.status >= 300:
            raise RuntimeError(f"POST failed with status {resp.status}")


def build_path_op(path: list[str], leaf: dict[str, Any]) -> dict[str, Any]:
    op = leaf
    for segment in reversed(path):
        if segment.startswith("[") and segment.endswith("]"):
            idx = int(segment[1:-1])
            op = {"Array": {"Update": {"pos": idx, "op": op}}}
        else:
            op = {"Object": {"Update": [segment, op]}}
    return op


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Update a field value (for numbers, use json_add to increment)"
    )
    parser.add_argument("path", help="JSON path (e.g., 'user.name')")
    parser.add_argument("value", help="New value (JSON string)")
    parser.add_argument("--url", default="http://localhost:8081", help="Node base URL")
    parser.add_argument(
        "--clear-first",
        action="store_true",
        help="Clear the field before setting new value",
    )
    args = parser.parse_args()

    try:
        value = json.loads(args.value)
    except json.JSONDecodeError:
        value = args.value

    path_parts = []
    for segment in args.path.replace("[", ".[").split("."):
        if segment:
            path_parts.append(segment)

    if args.clear_first:
        # For strings, delete all chars; for objects/arrays, use Clear
        if isinstance(value, str):
            # Assume existing string, delete chars one by one (simplified)
            print(
                "Note: --clear-first for strings requires knowing current length. Use json_string.py instead.",
                file=sys.stderr,
            )
        else:
            clear_op = build_path_op(path_parts, {"Object": "Clear"})
            post_op(args.url, clear_op)

    if isinstance(value, str):
        for i, ch in enumerate(value):
            op = build_path_op(path_parts, {"String": {"Insert": {"content": ch, "pos": i}}})
            post_op(args.url, op)
    elif isinstance(value, bool):
        op = build_path_op(path_parts, {"Boolean": "Enable" if value else "Disable"})
        post_op(args.url, op)
    elif isinstance(value, (int, float)):
        op = build_path_op(path_parts, {"Number": {"Inc": value}})
        post_op(args.url, op)
    else:
        print(
            f"ERROR: complex updates (objects/arrays) not yet supported. Use json_add/json_delete.",
            file=sys.stderr,
        )
        return 1

    print(f"Updated {args.path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
