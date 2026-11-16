#!/usr/bin/env python3
"""Validate a fixture against fixtures/schema/mac-main-sample.schema.json (required fields and types; stdlib only).

    python3 tools/check-fixture.py [fixture.json]      exit 0 valid, 1 with each problem listed
"""
import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TYPES = {"string": str, "integer": int, "boolean": bool, "array": list, "object": dict}


def check(value, schema, where, out):
    t = schema.get("type")
    if t and not (isinstance(value, TYPES[t]) and not (t == "integer" and isinstance(value, bool))):
        out.append("%s: expected %s, got %s" % (where, t, type(value).__name__))
        return
    if t == "object":
        for k in schema.get("required", []):
            if k not in value:
                out.append("%s: missing %s" % (where, k))
        for k, sub in schema.get("properties", {}).items():
            if k in value:
                check(value[k], sub, "%s.%s" % (where, k), out)
    if t == "array" and "items" in schema:
        for i, v in enumerate(value):
            check(v, schema["items"], "%s[%d]" % (where, i), out)


def main(argv):
    path = argv[0] if argv else os.path.join(ROOT, "docs", "design", "fixtures", "mac-main-sample.json")
    schema = json.load(open(os.path.join(ROOT, "fixtures", "schema", "mac-main-sample.schema.json")))
    problems = []
    check(json.load(open(path, encoding="utf-8")), schema, "$", problems)
    for p in problems:
        print(p)
    print("%s: %s" % (os.path.relpath(path, ROOT), "valid" if not problems else "%d problems" % len(problems)))
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
