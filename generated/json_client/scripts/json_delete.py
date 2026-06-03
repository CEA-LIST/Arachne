#!/usr/bin/env python3
"""Delete a field or array element from the JSON CRDT."""

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
    parser = argparse.ArgumentParser(description="Delete a field or array element")
    parser.add_argument("path", help="JSON path (e.g., 'user.name' or 'items[0]')")
    parser.add_argument("--url", default="http://localhost:8081", help="Node base URL")
    parser.add_argument("--array-pos", type=int, help="Delete array element at position")
    args = parser.parse_args()

    path_parts = []
    for segment in args.path.replace("[", ".[").split("."):
        if segment:
            path_parts.append(segment)

    if args.array_pos is not None:
        last = path_parts[-1]
        path_parts = path_parts[:-1]
        op = build_path_op(path_parts + [last], {"Array": {"Delete": {"pos": args.array_pos}}})
        post_op(args.url, op)
        print(f"Deleted array element at {args.path}[{args.array_pos}]")
    elif path_parts[-1].startswith("[") and path_parts[-1].endswith("]"):
        idx = int(path_parts[-1][1:-1])
        parent_path = path_parts[:-1]
        op = build_path_op(parent_path, {"Array": {"Delete": {"pos": idx}}})
        post_op(args.url, op)
        print(f"Deleted array element at {args.path}")
    else:
        parent_path = path_parts[:-1]
        key = path_parts[-1]
        if parent_path:
            op = build_path_op(parent_path, {"Object": {"Remove": key}})
        else:
            op = {"Object": {"Remove": key}}
        post_op(args.url, op)
        print(f"Deleted field {args.path}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
