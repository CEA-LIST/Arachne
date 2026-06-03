#!/usr/bin/env python3
"""Add a field or array element to the JSON CRDT."""

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


def value_to_op(value: Any, is_insert: bool = False, pos: int = 0) -> dict[str, Any]:
    if isinstance(value, str):
        if is_insert:
            return {"String": {"Insert": {"content": value[0] if value else " ", "pos": 0}}}
        return {"String": {"Insert": {"content": value, "pos": pos}}}
    if isinstance(value, bool):
        return {"Boolean": "Enable" if value else "Disable"}
    if isinstance(value, (int, float)):
        return {"Number": {"Inc": value}}
    if isinstance(value, dict):
        return {"Object": "Clear"}
    if isinstance(value, list):
        return {"Array": {"Insert": {"pos": 0, "op": {"Number": {"Inc": 0}}}}}
    raise ValueError(f"Unsupported value type: {type(value)}")


def main() -> int:
    parser = argparse.ArgumentParser(description="Add a field or array element")
    parser.add_argument("path", help="JSON path (e.g., 'user.name' or 'items[0]')")
    parser.add_argument("value", help="Value to add (JSON string)")
    parser.add_argument("--url", default="http://localhost:8081", help="Node base URL")
    parser.add_argument("--array-pos", type=int, help="Array position for insert")
    args = parser.parse_args()

    try:
        value = json.loads(args.value)
    except json.JSONDecodeError:
        value = args.value

    path_parts = []
    for segment in args.path.replace("[", ".[").split("."):
        if segment:
            path_parts.append(segment)

    if args.array_pos is not None:
        last = path_parts[-1]
        path_parts = path_parts[:-1]
        leaf_op = {"Array": {"Insert": {"pos": args.array_pos, "op": value_to_op(value, is_insert=True)}}}
        if isinstance(value, str) and len(value) > 1:
            post_op(args.url, build_path_op(path_parts + [last], leaf_op))
            for i, ch in enumerate(value[1:], start=1):
                update_op = {"Array": {"Update": {"pos": args.array_pos, "op": {"String": {"Insert": {"content": ch, "pos": i}}}}}}
                post_op(args.url, build_path_op(path_parts + [last], update_op))
        else:
            post_op(args.url, build_path_op(path_parts + [last], leaf_op))
        print(f"Added array element at {args.path}[{args.array_pos}]")
    else:
        if isinstance(value, str):
            for i, ch in enumerate(value):
                op = build_path_op(path_parts, {"String": {"Insert": {"content": ch, "pos": i}}})
                post_op(args.url, op)
        else:
            op = build_path_op(path_parts, value_to_op(value))
            post_op(args.url, op)
        print(f"Added {args.path}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
