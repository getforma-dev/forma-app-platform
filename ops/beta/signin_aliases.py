#!/usr/bin/env python3
"""Validate/copy one explicitly reviewed clean-trial routing alias, locally only.

This proves shape and continuity, not the handle's database ownership. The
operator/reviewer must compare the mapping with the retained successful
provision receipt for the existing clean-trial organization before deployment.
No credentials, provider calls, database reads or invitation changes occur here.
"""
import argparse
import base64
import json
from pathlib import Path
import re
import sys


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("alias configuration has duplicate JSON keys")
        result[key] = value
    return result


def document(path):
    raw = path.read_bytes()
    if len(raw) > 4096:
        raise ValueError("clean-trial alias file exceeds 4096 bytes")
    value = json.loads(raw, object_pairs_hook=unique_object)
    if not isinstance(value, dict) or set(value) != {"aliases"}:
        raise ValueError("clean-trial aliases must contain only aliases")
    entries = value["aliases"]
    if not isinstance(entries, list) or len(entries) != 1:
        raise ValueError("clean-trial aliases require exactly one reviewed mapping")
    entry = entries[0]
    if (not isinstance(entry, dict) or set(entry) != {"name", "handle"}
            or not isinstance(entry["name"], str) or not isinstance(entry["handle"], str)
            or not re.fullmatch(r"[a-z0-9](?:[a-z0-9-]{0,38}[a-z0-9])?", entry["name"])
            or not 2 <= len(entry["name"]) <= 40 or len(entry["name"]) == 26
            or not re.fullmatch(r"[a-z2-7]{26}", entry["handle"])):
        raise ValueError("clean-trial alias must be one valid readable name and canonical handle")
    decoded = base64.b32decode(entry["handle"].upper() + "======")
    if len(decoded) != 16 or base64.b32encode(decoded).decode("ascii").lower().rstrip("=") != entry["handle"]:
        raise ValueError("clean-trial alias handle must use canonical base32 padding bits")
    return value


def prepare(aliases, clean_trial, predecessor, legacy, output=None):
    prior = predecessor / "signin-aliases.json" if predecessor else None
    if aliases is None:
        if prior and prior.exists():
            raise ValueError("predecessor has aliases; pass --signin-aliases-file explicitly to preserve them")
        return
    if clean_trial is None or not clean_trial.is_file():
        raise ValueError("--signin-aliases-file requires --clean-trial-file")
    if aliases.resolve() == legacy.resolve():
        raise ValueError("clean trial refuses the shared legacy signin-aliases.json; use a reviewed dedicated file")
    value = document(aliases)
    if prior and prior.exists() and value != document(prior):
        raise ValueError("clean-trial redeploy must preserve its predecessor's alias mapping")
    if output:
        # Called only after the window is made. Never overwrite an existing
        # window's context or retain an external/symlink dependency for synth.
        with output.open("x", encoding="utf-8") as stream:
            json.dump(value, stream, sort_keys=True)
            stream.write("\n")
        output.chmod(0o600)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--aliases", type=Path)
    parser.add_argument("--clean-trial", type=Path)
    parser.add_argument("--predecessor", type=Path)
    parser.add_argument("--legacy", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        prepare(args.aliases, args.clean_trial, args.predecessor, args.legacy, args.output)
    except (ValueError, OSError) as error:
        print("alias configuration refused: " + str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
