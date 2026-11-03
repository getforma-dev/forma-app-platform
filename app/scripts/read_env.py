#!/usr/bin/env python3
"""APP-113: the one `.env` reader for the pitch's secret puts and smoke scripts.

It reads named variables from a dotenv file (the clone's gitignored `.env` by
default, `GENUI_ENV_FILE` or `--file` to override) and writes them to stdout
for a pipe, never to a terminal:

  read_env.py --raw ANTHROPIC_API_KEY
      the value alone, no newline (Secrets Manager stores exactly these bytes)
  read_env.py --object bot_token=SLACK_BOT_TOKEN signing_secret=SLACK_SIGNING_SECRET
      one compact JSON object whose fields are those variables' values

Parsing: blank lines and `#` comments are skipped; a leading `export ` is
dropped; a trailing `\\r` (a file saved with CRLF endings) is dropped; one pair
of matching surrounding quotes (single or double) is removed from the value; an
unquoted value ends at ` #` (an inline comment). A requested name that is
missing or empty, or that appears more than once, is refused, and so is a
value with a control character left in it. Errors name the variable and the
line number only, never a value.

`--optional` turns "missing or empty" into empty output and exit code 3 (so a
smoke script can skip); duplicates and bad values still refuse (exit 2).
Python callers use `read(path, names)` directly.
"""
import argparse
import json
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
NAME = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
ASSIGNMENT = re.compile(r"(?:export[ \t]+)?([A-Za-z_][A-Za-z0-9_]*)[ \t]*=(.*)")
MISSING = 3


class EnvError(ValueError):
    pass


def default_file():
    return Path(os.environ.get("GENUI_ENV_FILE") or ROOT / ".env")


def _value(raw, name, number):
    value = raw.strip(" \t")  # ASCII blanks only: a Unicode NEL or NBSP is not trimmed away
    if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
        value = value[1:-1]
    elif value and value[0] in "\"'":
        raise EnvError("%s (line %d): unbalanced quote" % (name, number))
    else:
        value = re.split(r"[ \t]+#", value, maxsplit=1)[0].rstrip(" \t")
    if any(ord(c) < 32 or 127 <= ord(c) < 160 for c in value):
        raise EnvError("%s (line %d): control character in the value" % (name, number))
    return value


def read(path, names):
    """{name: value} for each requested name present with a non-empty value.

    Refuses (EnvError) a requested name assigned more than once or a bad value.
    Names absent or empty are left out; the caller decides whether that is an error.
    """
    wanted = set(names)
    for name in wanted:
        if not NAME.fullmatch(name):
            raise EnvError("not a variable name: %r" % name)
    found, lines = {}, {}
    text = Path(path).read_text(encoding="utf-8")
    if text.startswith("﻿"):
        text = text[1:]
    for number, line in enumerate(text.split("\n"), 1):
        line = line[:-1] if line.endswith("\r") else line
        stripped = line.strip(" \t")
        if not stripped or stripped.startswith("#"):
            continue
        match = ASSIGNMENT.fullmatch(stripped)
        if not match or match.group(1) not in wanted:
            continue
        name = match.group(1)
        if name in lines:
            raise EnvError("%s is assigned more than once (lines %d and %d)" % (name, lines[name], number))
        lines[name] = number
        found[name] = _value(match.group(2), name, number)
    return {name: value for name, value in found.items() if value}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--file", type=Path, default=None, help="dotenv file (default: GENUI_ENV_FILE, else the clone's .env)")
    parser.add_argument("--optional", action="store_true", help="missing or empty: print nothing and exit 3")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--raw", metavar="NAME", help="print one value alone, with no newline")
    mode.add_argument("--object", nargs="+", metavar="FIELD=NAME", help="print one JSON object of FIELD: value of NAME")
    args = parser.parse_args(argv)
    path = args.file or default_file()
    if args.raw is not None:
        fields = {None: args.raw}
    else:
        fields = {}
        for pair in args.object:
            field, sep, name = pair.partition("=")
            if not sep or not field or field in fields:
                parser.error("--object takes distinct FIELD=NAME pairs")
            fields[field] = name
    if sys.stdout.isatty():
        print("read_env: refusing to print secret values to a terminal; pipe the output", file=sys.stderr)
        return 2
    try:
        if not path.is_file():
            raise FileNotFoundError
        values = read(path, fields.values())
    except FileNotFoundError:
        if args.optional:
            return MISSING
        print("read_env: no env file at %s" % path, file=sys.stderr)
        return 2
    except (EnvError, OSError, UnicodeDecodeError) as error:
        print("read_env: %s" % error, file=sys.stderr)
        return 2
    absent = sorted({name for name in fields.values() if name not in values})
    if absent:
        if args.optional:
            return MISSING
        print("read_env: missing or empty in %s: %s" % (path, ", ".join(absent)), file=sys.stderr)
        return 2
    if args.raw is not None:
        sys.stdout.write(values[args.raw])
    else:
        sys.stdout.write(json.dumps({field: values[name] for field, name in fields.items()}, separators=(",", ":")))
    sys.stdout.flush()
    return 0


if __name__ == "__main__":
    sys.exit(main())
