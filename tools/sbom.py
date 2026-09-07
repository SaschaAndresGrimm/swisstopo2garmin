#!/usr/bin/env python3
"""Generate a CycloneDX SBOM for a release (SPEC.md NFR-10).

Covers all four kinds of thing this application ships, which no single ecosystem tool
does:

* Rust crates, from `cargo metadata` -- the resolved graph, so it is what was actually
  built rather than what Cargo.toml asked for.
* npm packages, from `frontend/package-lock.json`.
* The vendored Java tools and the JRE, which are shipped as binaries inside the bundle
  and are invisible to both of the above. Leaving them out would be the SBOM's biggest
  omission: they are the largest components by size and the only GPL-2 ones.
* The swisstopo datasets, as `data` components. Not software, but a release that omits
  them would misrepresent what the application depends on, and their licence terms are
  the ones a redistributor most needs to know about.

CycloneDX 1.5 JSON, because it expresses non-software components and the licence field
takes an SPDX expression. Written by hand rather than with cargo-cyclonedx: one script
covering four ecosystems is less to keep working than four tools, and this one has no
dependencies beyond the Python standard library.

Usage:
    python3 tools/sbom.py > sbom.json
    python3 tools/sbom.py --check      # exit 1 if anything cannot be resolved
"""

import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent


def cargo_components() -> list[dict]:
    """Resolved Rust dependencies, with the licence each crate declares."""
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    meta = json.loads(out.stdout)
    workspace = set(meta["workspace_members"])
    components = []
    for pkg in meta["packages"]:
        if pkg["id"] in workspace:
            continue
        components.append(
            {
                "type": "library",
                "name": pkg["name"],
                "version": pkg["version"],
                "purl": f"pkg:cargo/{pkg['name']}@{pkg['version']}",
                "licenses": spdx(pkg.get("license")),
                "externalReferences": refs(pkg.get("repository")),
            }
        )
    return components


def npm_components() -> tuple[list[dict], int]:
    """Frontend dependencies that are actually shipped, and a count of those that are not.

    Dev-only packages -- eslint, vite, typescript, the whole babel tree -- are excluded.
    They are build tools: nothing of them reaches the bundle, and listing two hundred of
    them alongside React would bury the four components a redistributor has obligations
    about. The count is reported so the omission is visible rather than silent.

    npm's lock file records a licence for only some packages, so the installed
    `node_modules/<name>/package.json` is consulted as a fallback. When node_modules is
    absent (a clean checkout) the licence is NOASSERTION and `--check` says so, because
    an SBOM that guessed would be worse than one that admits the gap.
    """
    lock = ROOT / "frontend/package-lock.json"
    data = json.loads(lock.read_text())
    modules = ROOT / "frontend/node_modules"
    components, dev_only = [], 0

    for path, pkg in data.get("packages", {}).items():
        if not path or pkg.get("link"):
            continue
        if pkg.get("dev") or pkg.get("devOptional"):
            dev_only += 1
            continue
        name = path.split("node_modules/")[-1]
        version = pkg.get("version", "unknown")
        license_expr = pkg.get("license") or installed_license(modules / name)
        components.append(
            {
                "type": "library",
                "name": name,
                "version": version,
                "purl": f"pkg:npm/{name}@{version}",
                "licenses": spdx(license_expr),
                "externalReferences": refs(pkg.get("resolved")),
            }
        )
    return components, dev_only


def installed_license(pkg_dir: pathlib.Path) -> str | None:
    """The `license` field of an installed package, if it is installed."""
    manifest = pkg_dir / "package.json"
    if not manifest.is_file():
        return None
    try:
        data = json.loads(manifest.read_text())
    except (json.JSONDecodeError, OSError):
        return None
    lic = data.get("license")
    if isinstance(lic, str):
        return lic
    # The old object and array forms, still present in a few long-lived packages.
    if isinstance(lic, dict):
        return lic.get("type")
    if isinstance(data.get("licenses"), list) and data["licenses"]:
        first = data["licenses"][0]
        return first.get("type") if isinstance(first, dict) else None
    return None


def vendored_components() -> list[dict]:
    """The Java tools and the JRE, which ship inside the bundle.

    Versions are read from the directory names written by `fetch_tools.py`, so an SBOM
    generated after a tool update reports the new version without anybody editing this.
    Splitter is GPL-3.0-only, settled from its source headers rather than from the
    version this project's NOTICE used to assert; see the note there.
    """
    vendor = ROOT / "vendor"
    found = []

    def version_of(prefix: str) -> str | None:
        for child in sorted(vendor.glob(f"{prefix}-*")):
            if child.is_dir():
                return child.name.split("-", 1)[1]
        return None

    mkgmap = version_of("mkgmap")
    if mkgmap:
        found.append(
            {
                "type": "application",
                "name": "mkgmap",
                "version": mkgmap,
                "licenses": spdx("GPL-2.0-or-later"),
                "description": "Garmin map compiler, invoked as a separate process",
                "externalReferences": refs("https://www.mkgmap.org.uk/"),
            }
        )
    splitter = version_of("splitter")
    if splitter:
        found.append(
            {
                "type": "application",
                "name": "splitter",
                "version": splitter,
                "licenses": spdx("GPL-3.0-only"),
                "description": (
                    "OSM tile splitter, invoked as a separate process. GPL-3.0-only "
                    "per its own source headers; see NOTICE."
                ),
                "externalReferences": refs("https://www.mkgmap.org.uk/"),
            }
        )

    release = next(
        (
            p
            for p in (vendor / "jre" / "release", vendor / "jre" / "Contents/Home/release")
            if p.is_file()
        ),
        None,
    )
    if release:
        version = "unknown"
        for line in release.read_text().splitlines():
            if line.startswith("JAVA_VERSION="):
                version = line.split("=", 1)[1].strip().strip('"')
        found.append(
            {
                "type": "platform",
                "name": "Eclipse Temurin JRE",
                "version": version,
                "licenses": spdx("GPL-2.0-only WITH Classpath-exception-2.0"),
                "externalReferences": refs("https://adoptium.net/"),
            }
        )
    return found


def data_components() -> list[dict]:
    """The swisstopo datasets a build reads.

    Read from the source list in stac.rs, so a dataset added to the code appears here
    without anybody remembering to add it -- which is the whole reason to derive it.
    """
    text = (ROOT / "crates/s2g-core/src/stac.rs").read_text()
    ids = sorted(
        {
            line.split('"')[1]
            for line in text.splitlines()
            if line.startswith("pub const ") and 'str = "ch.' in line
        }
    )
    return [
        {
            "type": "data",
            "name": cid,
            "version": "resolved at download time from the STAC API",
            "licenses": [
                {
                    "license": {
                        "name": "swisstopo open government data — free use with "
                        "attribution",
                        "url": "https://www.swisstopo.admin.ch/en/"
                        "terms-of-use-free-geodata-and-geoservices",
                    }
                }
            ],
        }
        for cid in ids
    ]


def spdx(expr: str | None) -> list[dict]:
    if not expr:
        return [{"license": {"name": "NOASSERTION"}}]
    return [{"expression": expr}]


def refs(url: str | None) -> list[dict]:
    return [{"type": "website", "url": url}] if url else []


def main() -> int:
    check = "--check" in sys.argv

    # The version lives in the workspace manifest; src-tauri inherits it.
    app_version = "unknown"
    for line in (ROOT / "Cargo.toml").read_text().splitlines():
        if line.startswith("version = "):
            app_version = line.split('"')[1]
            break

    npm, dev_only = npm_components()
    groups = {
        "cargo": cargo_components(),
        "npm": npm,
        "vendored": vendored_components(),
        "data": data_components(),
    }

    sbom = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "metadata": {
            "component": {
                "type": "application",
                "name": "swisstopo2garmin",
                "version": app_version,
                "licenses": spdx("GPL-3.0-or-later"),
            },
            "tools": [{"name": "tools/sbom.py", "vendor": "swisstopo2garmin"}],
            "properties": [
                {
                    "name": "swisstopo2garmin:npm-dev-dependencies-excluded",
                    "value": str(dev_only),
                }
            ],
        },
        "components": [c for g in groups.values() for c in g],
    }

    if check:
        problems = []
        for kind, expected in (("vendored", 3), ("data", 10)):
            if len(groups[kind]) < expected:
                problems.append(
                    f"only {len(groups[kind])} {kind} component(s); expected at least "
                    f"{expected}. Run vendor/fetch_tools.py, or check that the source "
                    f"this is derived from still has the shape it is parsed from."
                )
        unknown = [
            c["name"]
            for c in groups["cargo"] + groups["npm"]
            if c["licenses"] == [{"license": {"name": "NOASSERTION"}}]
        ]
        if unknown:
            problems.append(
                f"{len(unknown)} dependency(ies) declare no licence: "
                f"{', '.join(sorted(unknown))}"
            )
        for p in problems:
            print(f"ERROR: {p}", file=sys.stderr)
        if problems:
            return 1
        print(
            "OK: "
            + ", ".join(f"{len(v)} {k}" for k, v in groups.items())
            + f" ({len(sbom['components'])} shipped components, "
            + f"{dev_only} npm dev-only excluded)"
        )
        return 0

    json.dump(sbom, sys.stdout, indent=2, ensure_ascii=False)
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
