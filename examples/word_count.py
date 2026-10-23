#!/usr/bin/env python3
"""Optional persistent-worker protocol demonstration, not a production plugin."""
import json
import sys

LIMIT = 48 * 1024 * 1024
while True:
    line = sys.stdin.buffer.readline(LIMIT + 1)
    if not line:
        break
    if len(line) > LIMIT or not line.endswith(b"\n"):
        raise ValueError("invalid worker frame")
    event = json.loads(line)
    ident = event["id"]
    try:
        text = event["data"]["arguments"]["text"]
        if not isinstance(text, str):
            raise ValueError("text must be a string")
        result = {"words": len(text.split()), "characters": len(text)}
        reply = {"id": ident, "data": {"output": json.dumps(result)}}
    except Exception as error:
        reply = {"id": ident, "error": str(error)}
    print(json.dumps(reply), flush=True)
