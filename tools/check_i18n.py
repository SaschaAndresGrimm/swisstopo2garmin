#!/usr/bin/env python3
"""Check the four language bundles against each other and against the components.

SPEC.md FR-4 requires de / fr / it / en with English as the fallback. The fallback is
what makes an incomplete bundle invisible: a missing German key silently renders the
English string, so a half-translated screen looks finished to anybody testing in
English. This is the only thing that catches it.

Three checks:

1. Every bundle has exactly the keys English has. A missing key falls back silently; an
   extra key is dead weight that suggests a rename went half-done.
2. Every literal key used in a component exists. A missing one renders as its own key
   name -- "about.trademarks" in the middle of a page -- which reviewers do notice, but
   only if they open that screen.
3. Every key in the bundle is used. Reported as a warning, not an error, because keys
   are also built from template literals (`preset.${id}`), and those prefixes are
   collected and treated as covering everything beneath them.

Run: python3 tools/check_i18n.py
"""

import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
I18N = ROOT / "frontend/src/i18n"
SRC = ROOT / "frontend/src"
LANGS = ["en", "de", "fr", "it"]

# t("literal.key") and t('literal.key')
LITERAL = re.compile(r"""\bt\(\s*["']([A-Za-z0-9_.]+)["']""")
# t(`prefix.${expr}`) -- the prefix covers every key under it
TEMPLATE = re.compile(r"""\bt\(\s*`([A-Za-z0-9_.]*?)\$\{""")
# Any dotted string literal. Some keys reach `t` through a variable rather than
# directly -- the Data screen's SOURCES table stores one per source -- and counting
# only direct calls reported twenty of those as unused.
REFERENCED = re.compile(r"""["']([a-z][A-Za-z0-9_]*(?:\.[A-Za-z0-9_]+)+)["']""")


def main() -> int:
    bundles = {lang: json.loads((I18N / f"{lang}.json").read_text()) for lang in LANGS}
    english = set(bundles["en"])
    problems = []

    for lang in LANGS[1:]:
        keys = set(bundles[lang])
        missing = sorted(english - keys)
        extra = sorted(keys - english)
        if missing:
            problems.append(
                f"{lang}.json is missing {len(missing)} key(s) English has, so they "
                f"fall back to English silently:\n    " + "\n    ".join(missing)
            )
        if extra:
            problems.append(
                f"{lang}.json has {len(extra)} key(s) English does not:\n    "
                + "\n    ".join(extra)
            )

    used, prefixes, referenced = set(), set(), set()
    for path in SRC.rglob("*.ts*"):
        text = path.read_text()
        used |= set(LITERAL.findall(text))
        prefixes |= set(TEMPLATE.findall(text))
        referenced |= set(REFERENCED.findall(text))

    undefined = sorted(k for k in used if k not in english)
    if undefined:
        problems.append(
            f"{len(undefined)} key(s) used in components but defined nowhere, which "
            f"render as their own name:\n    " + "\n    ".join(undefined)
        )

    unused = sorted(
        k
        for k in english
        if k not in used
        and k not in referenced
        and not any(k.startswith(p) for p in prefixes if p)
    )

    for p in problems:
        print(f"ERROR: {p}", file=sys.stderr)
    if unused:
        print(f"warning: {len(unused)} key(s) defined but never used: {', '.join(unused)}")

    if problems:
        return 1
    print(
        f"OK: {len(english)} keys x {len(LANGS)} languages, "
        f"{len(used)} literal uses, {len(prefixes)} template prefixes"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
