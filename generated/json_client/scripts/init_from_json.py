#!/usr/bin/env python3
"""Initialize json_crdt state by translating a plain JSON document into /api/op calls."""

from __future__ import annotations

import argparse
import json
import sys
import urllib.error
import urllib.request
from typing import Any

PathStep = tuple[str, Any]


def path_to_string(path: list[PathStep]) -> str:
    out = "$"
    for kind, value in path:
        if kind == "object":
            out += f".{value}"
        elif kind in {"array-insert", "array-update"}:
            out += f"[{value}]"
    return out


def wrap(path: list[PathStep], leaf: dict[str, Any]) -> dict[str, Any]:
    op: dict[str, Any] = leaf
    for kind, value in reversed(path):
        if kind == "object":
            op = {"Object": {"Update": [value, op]}}
        elif kind == "array-insert":
            op = {"Array": {"Insert": {"pos": value, "op": op}}}
        elif kind == "array-update":
            op = {"Array": {"Update": {"pos": value, "op": op}}}
        else:
            raise ValueError(f"Unsupported path step: {kind}")
    return op


def post_op(base_url: str, op: dict[str, Any], dry_run: bool) -> None:
    payload = {"JsonKind": op}
    if dry_run:
        print(json.dumps(payload, ensure_ascii=True))
        return

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


def emit_value(base_url: str, path: list[PathStep], value: Any, dry_run: bool) -> None:
    if isinstance(value, dict):
        if not value:
            post_op(base_url, wrap(path, {"Object": "Clear"}), dry_run)
            return
        for k, v in value.items():
            emit_value(base_url, path + [("object", k)], v, dry_run)
        return

    if isinstance(value, list):
        if not value:
            post_op(
                base_url,
                wrap(path, {"Array": {"Insert": {"pos": 0, "op": {"Number": {"Inc": 0}}}}}),
                dry_run,
            )
            post_op(base_url, wrap(path, {"Array": {"Delete": {"pos": 0}}}), dry_run)
            return
        for i, item in enumerate(value):
            emit_array_element(base_url, path, i, item, dry_run)
        return

    if isinstance(value, str):
        if value == "":
            post_op(
                base_url,
                wrap(path, {"String": {"Insert": {"content": " ", "pos": 0}}}),
                dry_run,
            )
            post_op(base_url, wrap(path, {"String": {"Delete": {"pos": 0}}}), dry_run)
            return
        for i, ch in enumerate(value):
            post_op(
                base_url,
                wrap(path, {"String": {"Insert": {"content": ch, "pos": i}}}),
                dry_run,
            )
        return

    if isinstance(value, bool):
        post_op(base_url, wrap(path, {"Boolean": "Enable" if value else "Disable"}), dry_run)
        return

    if isinstance(value, (int, float)):
        post_op(base_url, wrap(path, {"Number": {"Inc": value}}), dry_run)
        return

    if value is None:
        print(f"Skipping null at {path_to_string(path)}", file=sys.stderr)
        return

    raise ValueError(f"Unsupported JSON value type at path {path}: {type(value)}")


def emit_array_element(
    base_url: str,
    array_path: list[PathStep],
    index: int,
    value: Any,
    dry_run: bool,
) -> None:
    insert_path = array_path + [("array-insert", index)]
    update_path = array_path + [("array-update", index)]

    if isinstance(value, str):
        if value == "":
            post_op(
                base_url,
                wrap(insert_path, {"String": {"Insert": {"content": " ", "pos": 0}}}),
                dry_run,
            )
            post_op(base_url, wrap(update_path, {"String": {"Delete": {"pos": 0}}}), dry_run)
            return
        post_op(
            base_url,
            wrap(insert_path, {"String": {"Insert": {"content": value[0], "pos": 0}}}),
            dry_run,
        )
        for i, ch in enumerate(value[1:], start=1):
            post_op(
                base_url,
                wrap(update_path, {"String": {"Insert": {"content": ch, "pos": i}}}),
                dry_run,
            )
        return

    if isinstance(value, bool):
        post_op(
            base_url,
            wrap(insert_path, {"Boolean": "Enable" if value else "Disable"}),
            dry_run,
        )
        return

    if isinstance(value, (int, float)):
        post_op(base_url, wrap(insert_path, {"Number": {"Inc": value}}), dry_run)
        return

    if isinstance(value, dict):
        post_op(base_url, wrap(insert_path, {"Object": "Clear"}), dry_run)
        for k, v in value.items():
            emit_value(base_url, update_path + [("object", k)], v, dry_run)
        return

    if isinstance(value, list):
        post_op(
            base_url,
            wrap(
                insert_path,
                {"Array": {"Insert": {"pos": 0, "op": {"Number": {"Inc": 0}}}}},
            ),
            dry_run,
        )
        post_op(base_url, wrap(update_path, {"Array": {"Delete": {"pos": 0}}}), dry_run)
        for i, item in enumerate(value):
            emit_array_element(base_url, update_path, i, item, dry_run)
        return

    if value is None:
        print(
            f"Skipping null array element at {path_to_string(array_path)}[{index}]",
            file=sys.stderr,
        )
        return

    raise ValueError(f"Unsupported array element type at index {index}: {type(value)}")


def fetch_plain_state(base_url: str) -> dict[str, Any]:
    with urllib.request.urlopen(f"{base_url.rstrip('/')}/api/state", timeout=10) as resp:
        return json.loads(resp.read().decode("utf-8"))


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Initialize json_crdt by sending generated CRDT ops from a plain JSON file"
    )
    parser.add_argument("json_file", help="Path to input JSON file")
    parser.add_argument(
        "--url",
        default="http://localhost:8081",
        help="Base URL for the node (default: http://localhost:8081)",
    )
    parser.add_argument(
        "--clear-root",
        action="store_true",
        help="Send Object Clear before loading (root must be a JSON object)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print generated /api/op payloads instead of sending them",
    )
    args = parser.parse_args()

    try:
        with open(args.json_file, "r", encoding="utf-8") as fh:
            document = json.load(fh)
    except OSError as exc:
        print(f"ERROR: cannot read {args.json_file}: {exc}", file=sys.stderr)
        return 1
    except json.JSONDecodeError as exc:
        print(f"ERROR: invalid JSON in {args.json_file}: {exc}", file=sys.stderr)
        return 1

    if args.clear_root:
        if not isinstance(document, dict):
            print("ERROR: --clear-root currently supports object root only", file=sys.stderr)
            return 1
        try:
            post_op(args.url, {"Object": "Clear"}, args.dry_run)
        except Exception as exc:  # noqa: BLE001
            print(f"ERROR: failed to clear root: {exc}", file=sys.stderr)
            return 1

    try:
        emit_value(args.url, [], document, args.dry_run)
    except Exception as exc:  # noqa: BLE001
        print(f"ERROR: initialization failed: {exc}", file=sys.stderr)
        return 1

    if not args.dry_run:
        try:
            state = fetch_plain_state(args.url)
            print(json.dumps(state, ensure_ascii=True, indent=2))
        except Exception as exc:  # noqa: BLE001
            print(f"WARNING: initialization sent, but state fetch failed: {exc}", file=sys.stderr)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
