#!/usr/bin/env python3
"""The module dependency report of docs/architecture/MODULE-MAP.md (ADR-013 D13.4).

Walks the release sources of app/src (the module tree minus `#[cfg(test)]`
modules, the inline test module at a file's tail, files gated with an inner
`#![cfg(...)]`, and the `dev-stand` dev module and binary: D13.4 rule 3
exempts test and dev code), collects every `crate::<module>` a module names,
and compares the edges with the four-layer table of D13.4.

Report only: it exits 0 whatever it finds. Enforcement (`just check deps`, an
allowlist that fails on a new edge) is deferred to APP-097 by ADR-013.

  python3 scripts/dev/deps.py            the matrix, then the edges against the rules
  python3 scripts/dev/deps.py --json     the same as JSON
"""
import json
import re
import sys
from pathlib import Path

SRC = Path(__file__).resolve().parents[2] / "app/src"
# ADR-013 D13.4. `main` and `bin/*` are composition (layer 4).
LAYERS = {
    0: ["config", "roles", "readiness", "supervise", "origins", "directory", "demo", "http"],
    1: ["storage", "auth", "admission"],
    2: ["query", "composer", "actions", "commands", "boxes", "sharing", "widgets", "imports", "events"],
    3: ["pages", "mcp", "slack"],
    4: ["product", "main"],
}
LAYER = {module: layer for layer, modules in LAYERS.items() for module in modules}
EXEMPT = {"dev", "auth_qualification"}  # dev-stand and the qualification harness (rule 3)
TEST_DECL = re.compile(r'#\[cfg\((?:test|feature = "dev-stand")\)\]\s*(?:#\[path = "([^"]+)"\]\s*)?(?:pub(?:\(crate\))? )?mod (\w+);')
ANY_DECL = re.compile(r'(?:#\[path = "([^"]+)"\]\s*)?(?:pub(?:\(crate\))? )?mod (\w+);')
INLINE_TEST = re.compile(r"#\[cfg\(test\)\]\s*(?:pub(?:\(crate\))? )?mod \w+ \{")


def declared(path, pattern):
    base = path.parent if path.name in ("mod.rs", "lib.rs", "main.rs") else path.parent / path.stem
    for custom, name in pattern.findall(path.read_text()):
        yield (path.parent / custom).resolve() if custom else (base / (name + ".rs")).resolve()
        yield (base / name).resolve()


def gated_files():
    """Files declared behind cfg(test)/dev-stand, everything below them, and
    (transitively) every module a gated file declares."""
    gated = set()
    for path in SRC.rglob("*.rs"):
        gated.update(declared(path, TEST_DECL))
    pending = [p for p in gated if p.is_file()]
    while pending:
        for child in declared(pending.pop(), ANY_DECL):
            if child not in gated:
                gated.add(child)
                if child.is_file():
                    pending.append(child)
    return gated


def module_of(path):
    relative = path.relative_to(SRC)
    if relative.parts[0] == "bin":
        return None  # binaries: composition
    return relative.parts[0].removesuffix(".rs")


def names(text):
    """Every top-level module named as crate::<m> or inside use crate::{...}."""
    found = set(re.findall(r"\bcrate::(\w+)", text))
    for match in re.finditer(r"\bcrate::\{", text):
        depth, start, i = 1, match.end(), match.end()
        while depth and i < len(text):
            depth += {"{": 1, "}": -1}.get(text[i], 0)
            i += 1
        segment, level = "", 0
        for ch in text[start:i - 1] + ",":
            if ch == "{":
                level += 1
            elif ch == "}":
                level -= 1
            if ch == "," and level == 0:
                head = re.match(r"\s*(\w+)", segment)
                if head:
                    found.add(head.group(1))
                segment = ""
            else:
                segment += ch
    return found


def matrix():
    gated = gated_files()
    edges = {}
    for path in sorted(SRC.rglob("*.rs")):
        resolved = path.resolve()
        if resolved in gated or any(parent in gated for parent in resolved.parents):
            continue
        module = module_of(path)
        if module is None or module in EXEMPT or module == "lib":
            continue
        text = path.read_text()
        if re.search(r"^\s*#!\[cfg\(", text, re.M):
            continue
        cut = INLINE_TEST.search(text)
        if cut:
            text = text[:cut.start()]
        for name in names(text):
            if name != module and name in LAYER | {m: 0 for m in EXEMPT}:
                edges.setdefault(module, set()).add(name)
    return {module: sorted(deps) for module, deps in sorted(edges.items())}


def judge(edges):
    """Each edge against D13.4: a lower layer, or a sibling inside layer 2,
    or the kept slack -> mcp exception; layer 4 may name anything."""
    verdicts = []
    for module, deps in edges.items():
        own = LAYER.get(module)
        for dep in deps:
            target = LAYER.get(dep)
            if own is None or target is None:
                verdict = "unclassified"
            elif own == 4 or target < own or (own == target == 2):
                verdict = "allowed"
            elif (module, dep) == ("slack", "mcp"):
                verdict = "exception (D13.4: kept)"
            else:
                verdict = "violation"
            verdicts.append({"from": module, "from_layer": own, "to": dep, "to_layer": target, "verdict": verdict})
    return verdicts


def main():
    edges = matrix()
    verdicts = judge(edges)
    if "--json" in sys.argv:
        print(json.dumps({"edges": edges, "verdicts": verdicts}, indent=2))
        return 0
    print("Release-code dependencies (crate::<module>), cfg(test) and dev-stand code excluded:")
    for module, deps in edges.items():
        print("  %-10s L%s -> %s" % (module, LAYER.get(module, "?"), " ".join(deps)))
    bad = [v for v in verdicts if v["verdict"] != "allowed"]
    print("\nEdges outside the D13.4 rules (%d):" % len(bad))
    for v in bad:
        print("  %-10s (L%s) -> %-10s (L%s)  %s" % (v["from"], v["from_layer"], v["to"], v["to_layer"], v["verdict"]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
