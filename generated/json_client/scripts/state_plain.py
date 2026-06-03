#!/usr/bin/env python3
"""Fetch /api/state and decode CRDT wrappers into plain JSON."""

from __future__ import annotations

import argparse
import json
import sys
import urllib.error
import urllib.request
from typing import Any


def decode(node: Any) -> Any:
    if isinstance(node, dict):
        if "Value" in node:
            return decode(node["Value"])
        if "Object" in node:
            obj = node["Object"]
            if isinstance(obj, dict):
                return {k: decode(v) for k, v in obj.items()}
            return decode(obj)
        if "Array" in node:
            arr = node["Array"]
            if isinstance(arr, list):
                return [decode(v) for v in arr]
            return decode(arr)
        if "String" in node:
            s = node["String"]
            if isinstance(s, list):
                return "".join(str(ch) for ch in s)
            return s
        if "Number" in node:
            num = node["Number"]
            if isinstance(num, float) and num.is_integer():
                return int(num)
            return num
        if "Boolean" in node:
            return bool(node["Boolean"])
        return {k: decode(v) for k, v in node.items()}
    if isinstance(node, list):
        return [decode(v) for v in node]
    return node


def main() -> int:
    parser = argparse.ArgumentParser(description="Decode JSON CRDT state into plain JSON")
    parser.add_argument(
        "--url",
        default="http://localhost:8081/api/state",
        help="State endpoint URL (default: http://localhost:8081/api/state)",
    )
    args = parser.parse_args()

    try:
        with urllib.request.urlopen(args.url, timeout=10) as resp:
            payload = json.loads(resp.read().decode("utf-8"))
    except urllib.error.URLError as exc:
        print(f"ERROR: cannot fetch state from {args.url}: {exc}", file=sys.stderr)
        return 1

    root = payload.get("json", payload)
    plain = decode(root)
    print(json.dumps(plain, ensure_ascii=True, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
