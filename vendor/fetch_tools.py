"""Fetch the pinned Garmin toolchain and a portable JRE (SPEC.md §7.8, FR-P11/FR-P12).

Nothing is installed system-wide. Everything lands under vendor/ and is gitignored.
Versions are pinned here; bumping them is a deliberate, reviewable change.
"""
from __future__ import annotations

import hashlib
import io
import os
import platform
import shutil
import stat
import tarfile
import urllib.request
import zipfile
from pathlib import Path

VENDOR = Path(__file__).resolve().parent

MKGMAP = "r4924"
SPLITTER = "r654"
JAVA_FEATURE = "17"          # mkgmap needs Java 8+; 17 LTS is what we ship
# "jre" is what we ship to users; "jdk" adds javac/javap, needed to build the
# IMG dump tool used by the render harness (S2G_JAVA_IMAGE=jdk).
JAVA_IMAGE = os.environ.get("S2G_JAVA_IMAGE", "jre")

MKGMAP_URL = f"https://www.mkgmap.org.uk/download/mkgmap-{MKGMAP}.zip"
SPLITTER_URL = f"https://www.mkgmap.org.uk/download/splitter-{SPLITTER}.zip"


def adoptium_url() -> str:
    m = platform.machine().lower()
    arch = {"arm64": "aarch64", "aarch64": "aarch64",
            "x86_64": "x64", "amd64": "x64"}.get(m)
    if arch is None:
        raise SystemExit(f"unsupported architecture: {m}")
    osname = {"Darwin": "mac", "Linux": "linux", "Windows": "windows"}[platform.system()]
    return (f"https://api.adoptium.net/v3/binary/latest/{JAVA_FEATURE}/ga/"
            f"{osname}/{arch}/{JAVA_IMAGE}/hotspot/normal/eclipse")


def download(url: str) -> bytes:
    print(f"  fetching {url}")
    req = urllib.request.Request(url, headers={"User-Agent": "swisstopo2garmin/0"})
    with urllib.request.urlopen(req, timeout=300) as r:
        data = r.read()
    print(f"    {len(data):,} bytes  sha256 {hashlib.sha256(data).hexdigest()[:16]}...")
    return data


def unpack_zip(data: bytes, dest: Path) -> None:
    with zipfile.ZipFile(io.BytesIO(data)) as z:
        z.extractall(dest)


def unpack_tgz(data: bytes, dest: Path) -> None:
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as t:
        t.extractall(dest)


def unpack_archive(data: bytes, dest: Path) -> None:
    """Unpack whichever archive Adoptium served.

    It ships the Windows JRE as a **zip** and every other platform as a tar.gz, so
    unpacking unconditionally as a tarball meant this script had never once succeeded on
    Windows -- and therefore that the Windows installer NFR-7 promises had never been
    built. CI on `windows-latest` is what finally said so.

    Sniffed from the bytes rather than from `platform.system()`: the file's own magic is
    the thing that decides how to read it, and it stays right if Adoptium ever changes
    which platform gets which.
    """
    if data[:2] == b"PK":
        unpack_zip(data, dest)
    else:
        unpack_tgz(data, dest)


def ensure_jars() -> tuple[Path, Path]:
    for name, url, ver in (("mkgmap", MKGMAP_URL, MKGMAP), ("splitter", SPLITTER_URL, SPLITTER)):
        target = VENDOR / f"{name}-{ver}"
        jar = target / f"{name}.jar"
        if jar.exists():
            print(f"  {name} {ver}: present")
            continue
        print(f"  {name} {ver}:")
        tmp = VENDOR / f".{name}.tmp"
        shutil.rmtree(tmp, ignore_errors=True)
        unpack_zip(download(url), tmp)
        inner = next(p for p in tmp.iterdir() if p.is_dir())
        shutil.rmtree(target, ignore_errors=True)
        inner.rename(target)
        shutil.rmtree(tmp, ignore_errors=True)
        if not jar.exists():
            raise SystemExit(f"{jar} missing after unpack")
    return (VENDOR / f"mkgmap-{MKGMAP}" / "mkgmap.jar",
            VENDOR / f"splitter-{SPLITTER}" / "splitter.jar")


def ensure_java() -> Path:
    marker = VENDOR / JAVA_IMAGE
    if marker.exists():
        java = find_java(marker)
        if java:
            print(f"  jre: present ({java})")
            return java
    print(f"  {JAVA_IMAGE} (Temurin {JAVA_FEATURE}):")
    data = download(adoptium_url())
    tmp = VENDOR / f".{JAVA_IMAGE}.tmp"
    shutil.rmtree(tmp, ignore_errors=True)
    tmp.mkdir(parents=True)
    unpack_archive(data, tmp)
    inner = next(p for p in tmp.iterdir() if p.is_dir())
    shutil.rmtree(marker, ignore_errors=True)
    inner.rename(marker)
    shutil.rmtree(tmp, ignore_errors=True)
    java = find_java(marker)
    if not java:
        raise SystemExit(f"no java binary found under {marker}")
    java.chmod(java.stat().st_mode | stat.S_IEXEC)
    make_writable(marker)
    return java


def make_writable(root: Path) -> None:
    """Give the owner write permission on everything under `root`.

    Temurin ships 430 read-only files -- the CDS archives under `lib/server`, and every
    licence text. Tauri's bundler copies declared resources into `target/` preserving
    their mode, so the second build after the JRE is declared a resource fails with
    "Permission denied" trying to overwrite a file it wrote itself and then could not
    touch. Fixing it at the source is a line here; working around it would mean not
    shipping the JRE with the app.
    """
    fixed = 0
    for path in root.rglob("*"):
        try:
            mode = path.stat().st_mode
        except OSError:
            continue
        if not mode & stat.S_IWUSR:
            try:
                path.chmod(mode | stat.S_IWUSR)
                fixed += 1
            except OSError:
                pass
    if fixed:
        print(f"  made {fixed} read-only file(s) writable (see make_writable)")


def find_java(root: Path) -> Path | None:
    """The java launcher inside an unpacked JRE, whatever shape it came in.

    Three layouts: `bin/java` on Linux, `Contents/Home/bin/java` in a macOS bundle, and
    `bin/java.exe` on Windows. The Windows name was missing, which is the second reason
    this script had never worked there -- and it would have failed *after* a 130 MB
    download, reported as "no java binary found".
    """
    names = ("java", "java.exe")
    for parent in (root / "bin", root / "Contents" / "Home" / "bin"):
        for name in names:
            if (parent / name).exists():
                return parent / name
    for name in names:
        hits = list(root.rglob(f"bin/{name}"))
        if hits:
            return hits[0]
    return None


def main() -> None:
    print("vendoring Garmin toolchain into", VENDOR)
    mkgmap, splitter = ensure_jars()
    java = ensure_java()
    env = VENDOR / "toolchain.env"
    env.write_text(
        f"MKGMAP_JAR={mkgmap}\nSPLITTER_JAR={splitter}\nJAVA_BIN={java}\n"
        f"MKGMAP_VERSION={MKGMAP}\nSPLITTER_VERSION={SPLITTER}\n"
    )
    print(f"\nwrote {env}")
    os.execv(str(java), [str(java), "-version"])


if __name__ == "__main__":
    main()
