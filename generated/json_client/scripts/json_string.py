#!/usr/bin/env python3
"""String-specific operations for JSON CRDT (append, insert, delete, replace)."""

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


def fetch_state(base_url: str) -> dict[str, Any]:
    with urllib.request.urlopen(f"{base_url.rstrip('/')}/api/state", timeout=10) as resp:
        return json.loads(resp.read().decode("utf-8"))


def decode_string_at_path(state: dict[str, Any], path: list[str]) -> str:
    node = state.get("json", state)
    for segment in path:
        if segment.startswith("[") and segment.endswith("]"):
            idx = int(segment[1:-1])
            if "Value" in node:
                node = node["Value"]
            if "Array" in node:
                node = node["Array"][idx]
            else:
                return ""
        else:
            if "Value" in node:
                node = node["Value"]
            if "Object" in node:
                node = node["Object"].get(segment, {})
            else:
                return ""
    if "Value" in node:
        node = node["Value"]
    if "String" in node:
        return "".join(str(ch) for ch in node["String"])
    return ""


def main() -> int:
    parser = argparse.ArgumentParser(description="String-specific JSON CRDT operations")
    parser.add_argument("command", choices=["append", "insert", "delete", "replace", "clear"])
    parser.add_argument("path", help="JSON path to the string field (e.g., 'user.name')")
    parser.add_argument("value", nargs="?", help="Text value (for append, insert, replace)")
    parser.add_argument("--pos", type=int, help="Position for insert/delete operations")
    parser.add_argument("--len", type=int, help="Length for delete or replace operations")
    parser.add_argument("--url", default="http://localhost:8081", help="Node base URL")
    args = parser.parse_args()

    path_parts = []
    for segment in args.path.replace("[", ".[").split("."):
        if segment:
            path_parts.append(segment)

    if args.command == "append":
        if not args.value:
            print("ERROR: append requires a value", file=sys.stderr)
            return 1
        try:
            state = fetch_state(args.url)
            current = decode_string_at_path(state, path_parts)
            pos = len(current)
        except Exception as exc:
            print(f"WARNING: could not fetch current state: {exc}", file=sys.stderr)
            print("Assuming empty string (pos=0)", file=sys.stderr)
            pos = 0

        for i, ch in enumerate(args.value):
            op = build_path_op(path_parts, {"String": {"Insert": {"content": ch, "pos": pos + i}}})
            post_op(args.url, op)
        print(f"Appended '{args.value}' to {args.path}")

    elif args.command == "insert":
        if not args.value or args.pos is None:
            print("ERROR: insert requires --pos and a value", file=sys.stderr)
            return 1
        for i, ch in enumerate(args.value):
            op = build_path_op(
                path_parts, {"String": {"Insert": {"content": ch, "pos": args.pos + i}}}
            )
            post_op(args.url, op)
        print(f"Inserted '{args.value}' at {args.path}[{args.pos}]")

    elif args.command == "delete":
        if args.pos is None:
            print("ERROR: delete requires --pos", file=sys.stderr)
            return 1
        if args.len:
            op = build_path_op(
                path_parts, {"String": {"DeleteRange": {"start": args.pos, "len": args.len}}}
            )
            post_op(args.url, op)
            print(f"Deleted {args.len} chars from {args.path}[{args.pos}]")
        else:
            op = build_path_op(path_parts, {"String": {"Delete": {"pos": args.pos}}})
            post_op(args.url, op)
            print(f"Deleted char at {args.path}[{args.pos}]")

    elif args.command == "replace":
        if not args.value or args.pos is None or args.len is None:
            print("ERROR: replace requires --pos, --len, and a value", file=sys.stderr)
            return 1
        del_op = build_path_op(
            path_parts, {"String": {"DeleteRange": {"start": args.pos, "len": args.len}}}
        )
        post_op(args.url, del_op)
        for i, ch in enumerate(args.value):
            ins_op = build_path_op(
                path_parts, {"String": {"Insert": {"content": ch, "pos": args.pos + i}}}
            )
            post_op(args.url, ins_op)
        print(f"Replaced {args.len} chars at {args.path}[{args.pos}] with '{args.value}'")

    elif args.command == "clear":
        try:
            state = fetch_state(args.url)
            current = decode_string_at_path(state, path_parts)
            length = len(current)
            if length > 0:
                op = build_path_op(path_parts, {"String": {"DeleteRange": {"start": 0, "len": length}}})
                post_op(args.url, op)
                print(f"Cleared {args.path} ({length} chars)")
            else:
                print(f"{args.path} is already empty")
        except Exception as exc:
            print(f"ERROR: could not clear string: {exc}", file=sys.stderr)
            return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
