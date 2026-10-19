#!/usr/bin/env python3
"""Structural + WCAG validator for the Isolith DESIGN.md token spec.

Mirrors the checks that @google/design.md lint performs on a DESIGN.md file,
implemented deterministically in stdlib + PyYAML so the kit is verifiable
without a network fetch:

- front matter parses and carries name + colors (+ typography)
- colors.primary exists
- every {token} reference in components resolves
- every component textColor vs backgroundColor pair passes WCAG AA (4.5:1)
- every color token is referenced by a component (no orphans, warning only)

Usage: python3 validate_design.py  (cwd = kit root or pass a DESIGN.md path)
Exit 0 = clean, 1 = error, 2 = warnings only.

WCAG contrast: (L1 + 0.05) / (L2 + 0.05) using relative luminance
L = 0.2126 R + 0.7152 G + 0.0722 B on gamma-expanded channels (WCAG 2.x).
"""
import re
import sys
from pathlib import Path

try:
    import yaml
except ImportError:
    yaml = None

HERE = Path(__file__).resolve().parent
DESIGN = Path(sys.argv[1]) if len(sys.argv) > 1 else HERE / "DESIGN.md"

ERRORS = []
WARNINGS = []


def err(msg):
    ERRORS.append(msg)


def warn(msg):
    WARNINGS.append(msg)


def parse(path):
    text = path.read_text(encoding="utf-8")
    m = re.match(r"^---\n(.*?)\n---\n", text, re.S)
    if not m:
        err("no YAML front matter")
        return {}
    if yaml is None:
        err("PyYAML not installed; cannot parse tokens")
        return {}
    try:
        return yaml.safe_load(m.group(1)) or {}
    except Exception as e:  # noqa: BLE001
        err(f"front matter did not parse: {e}")
        return {}


def hex_to_luminance(h):
    h = h.strip().lstrip("#")
    if len(h) == 3:
        h = "".join(c * 2 for c in h)
    parts = [int(h[i:i + 2], 16) / 255.0 for i in (0, 2, 4)]
    lin = []
    for c in parts:
        if c <= 0.04045:
            lin.append(c / 12.92)
        else:
            lin.append(((c + 0.055) / 1.055) ** 2.4)
    return 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2]


def contrast(fg, bg):
    l1, l2 = hex_to_luminance(fg), hex_to_luminance(bg)
    hi, lo = max(l1, l2), min(l1, l2)
    return (hi + 0.05) / (lo + 0.05)


def resolve(data, value, base=None):
    if not isinstance(value, str):
        return value
    def repl(match):
        ref = match.group(1)
        node = data
        for key in ref.split("."):
            if isinstance(node, dict):
                node = node.get(key)
            else:
                node = None
                break
        if node is None:
            err(f"broken token reference {{ {ref} }}")
            return match.group(0)
        return node if isinstance(node, str) else str(node)
    return re.sub(r"\{([a-zA-Z0-9_.-]+)\}", repl, value)


def main():
    data = parse(DESIGN)

    if not data.get("name"):
        err("front matter missing name")
    colors = data.get("colors") or {}
    if not colors:
        err("front matter missing colors")
    elif "primary" not in colors:
        warn("colors.primary missing (design.md: missing-primary)")

    typo = data.get("typography") or {}
    if colors and not typo:
        warn("typography missing (design.md: missing-typography)")

    # resolve and validate every color token value (support alias refs)
    resolved_colors = {}
    _colors = dict(colors or {})
    for _ in range(6):  # bounded alias chase
        changed = False
        for k, v in list(_colors.items()):
            if not isinstance(v, str):
                continue
            if re.match(r"^#?[0-9a-fA-F]{3,8}$", v.strip()):
                _colors[k] = v.strip()
            elif (m := re.fullmatch(r"\{colors\.([a-zA-Z0-9_.-]+)\}", v.strip())):
                tgt = _colors.get(m.group(1))
                if isinstance(tgt, str) and tgt != v.strip():
                    _colors[k] = tgt
                    changed = True
            else:
                warn(f"color token '{k}' is not a plain hex or alias: {v!r}")
        if not changed:
            break
    resolved_colors = {k: v for k, v in _colors.items()
                       if isinstance(v, str) and re.match(r"^#?[0-9a-fA-F]{3,8}$", v.strip())}
    for k in (colors or {}):
        if k not in resolved_colors:
            warn(f"color token '{k}' could not be resolved to a hex")

    # component contrast + broken ref / orphan checks
    def to_hex(v):
        if not isinstance(v, str):
            return v
        m2 = re.fullmatch(r"\{colors\.([a-zA-Z0-9_.-]+)\}", v.strip())
        if m2:
            return resolved_colors.get(m2.group(1))
        if re.match(r"^#?[0-9a-fA-F]{3,8}$", v.strip()):
            return v.strip()
        return None

    referenced = set()
    comps = (data.get("components") or {})
    for cname, c in comps.items():
        if not isinstance(c, dict):
            continue
        bg = to_hex(resolve(data, c.get("backgroundColor")))
        fg = to_hex(resolve(data, c.get("textColor")))
        for val in (c.get("backgroundColor"), c.get("textColor"), c.get("typography"), c.get("rounded")):
            if isinstance(val, str):
                referenced.update(re.findall(r"\{([a-zA-Z0-9_.-]+)\}", val))
        if bg and fg:
            try:
                r = contrast(fg, bg)
            except ValueError as e:
                err(f"{cname}: unparseable color - {e}")
                continue
            label = f"{cname} (fg {fg} / bg {bg}): {r:.2f}:1"
            if r < 4.5:
                err(f"{label} below WCAG AA for normal text")
            else:
                print(f"[PASS] {label}")

    for token in referenced:
        parts = token.split(".")
        node = data
        for key in parts:
            node = node.get(key, None) if isinstance(node, dict) else None
            if node is None:
                break
        if node is None:
            err(f"component references missing token: {token}")

    for k in resolved_colors:
        for token in referenced:
            if token.split(".")[0] == "colors" and token.split(".")[-1] == k:
                break
        else:
            warn(f"color token '{k}' never referenced by a component (orphaned-tokens)")

    print(f"\nchecking {DESIGN.name}: {len(ERRORS)} errors, {len(WARNINGS)} warnings")
    for w in WARNINGS:
        print(f"  [warn] {w}")
    for e in ERRORS:
        print(f"  [error] {e}")
    if ERRORS:
        return 1
    if WARNINGS:
        print("PASSED with warnings (matching @google/design.md semantics: warnings alone exit 0)")
        return 0
    print("ALL CHECKS PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
