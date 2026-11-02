#!/usr/bin/env python3
"""Package an already built binary as a .deb, without dpkg, root or extra tools.

Usage: python3 scripts/build-deb.py <binary> <output-dir>
Reads [package] and [package.metadata.deb] from Cargo.toml, works out Depends
from the binary's ELF metadata, and prints the path of the .deb it wrote.
"""
import hashlib
import io
import lzma
import math
import os
import platform
import re
import subprocess
import sys
import tarfile
import time
import tomllib
import textwrap
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

ARCHITECTURES = {
    "x86_64": "amd64",
    "aarch64": "arm64",
    "armv7l": "armhf",
    "i686": "i386",
    "riscv64": "riscv64",
    "ppc64le": "ppc64el",
    "s390x": "s390x",
}

# Shared libraries a Rust binary can link, and the Debian package that ships each one.
LIBRARY_PACKAGES = {
    "libc.so.6": "libc6",
    "libm.so.6": "libc6",
    "libpthread.so.0": "libc6",
    "libdl.so.2": "libc6",
    "librt.so.1": "libc6",
    "libutil.so.1": "libc6",
    "ld-linux-x86-64.so.2": "libc6",
    "ld-linux-aarch64.so.1": "libc6",
    "libgcc_s.so.1": "libgcc-s1 | libgcc1",
}


def readelf(binary, option):
    return subprocess.check_output(
        ["readelf", "--wide", option, str(binary)],
        text=True,
        env={**os.environ, "LC_ALL": "C"},
    )


def depends(binary):
    needed = re.findall(r"\(NEEDED\).*?\[([^\]]+)\]", readelf(binary, "--dynamic"))
    unknown = [lib for lib in needed if lib not in LIBRARY_PACKAGES]
    if unknown:
        raise SystemExit(f"No Debian package known for {', '.join(unknown)}; add it to LIBRARY_PACKAGES")
    # Weak symbol versions are optional at run time, so they do not raise the libc6 floor.
    glibc = [
        tuple(int(part) for part in version.split("."))
        for version, flags in re.findall(r"Name:\s+GLIBC_([0-9.]+)\s+Flags:\s+(\S+)", readelf(binary, "--version-info"))
        if flags != "WEAK"
    ]
    result = []
    for library in needed:
        package = LIBRARY_PACKAGES[library]
        if package == "libc6" and glibc:
            package = f"libc6 (>= {'.'.join(map(str, max(glibc)))})"
        if not any(entry.split(" ")[0] == package.split(" ")[0] for entry in result):
            result.append(package)
    return ", ".join(result)


# Debian names for SPDX licenses whose full text every Debian system already ships.
COMMON_LICENSES = {
    "GPL-3.0-or-later": ("GPL-3+", "GPL-3", "version 3 of the License, or\n (at your option) any later version"),
    "GPL-3.0-only": ("GPL-3", "GPL-3", "version 3 of the License"),
}


def copyright_file(package, homepage, spdx, holder):
    if spdx not in COMMON_LICENSES:
        raise SystemExit(f"Add the Debian copyright text for license {spdx} to COMMON_LICENSES")
    short, common, versions = COMMON_LICENSES[spdx]
    return (
        "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n"
        f"Upstream-Name: {package}\n"
        f"Source: {homepage}\n\n"
        "Files: *\n"
        f"Copyright: {holder}\n"
        f"License: {short}\n\n"
        f"License: {short}\n"
        " This program is free software: you can redistribute it and/or modify\n"
        " it under the terms of the GNU General Public License as published by\n"
        f" the Free Software Foundation, either {versions}.\n"
        " .\n"
        " This program is distributed in the hope that it will be useful,\n"
        " but WITHOUT ANY WARRANTY; without even the implied warranty of\n"
        " MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the\n"
        " GNU General Public License for more details.\n"
        " .\n"
        " On Debian systems, the complete text of the GNU General Public License\n"
        f" version 3 can be found in /usr/share/common-licenses/{common}.\n"
    ).encode()


def tar(entries, mtime):
    """entries: list of (path, bytes or None for a folder, mode)."""
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w:xz", format=tarfile.GNU_FORMAT, preset=9 | lzma.PRESET_EXTREME) as archive:
        for path, data, mode in sorted(entries, key=lambda e: e[0]):
            info = tarfile.TarInfo(path)
            info.uid = info.gid = 0
            info.uname = info.gname = "root"
            info.mtime = mtime
            info.mode = mode
            if data is None:
                info.type = tarfile.DIRTYPE
                archive.addfile(info)
            else:
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))
    return buffer.getvalue()


def ar(members, mtime):
    out = bytearray(b"!<arch>\n")
    for name, data in members:
        header = f"{name:<16}{mtime:<12}{0:<6}{0:<6}{100644:<8}{len(data):<10}`\n"
        out += header.encode()
        out += data
        if len(data) % 2:
            out += b"\n"
    return bytes(out)


def main():
    binary, output = Path(sys.argv[1]), Path(sys.argv[2])
    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text())
    package = cargo["package"]
    meta = package.get("metadata", {}).get("deb", {})
    name = package["name"]
    revision = os.environ.get("DEB_REVISION", meta.get("revision", "1"))
    version = f"{package['version']}-{revision}"
    arch = ARCHITECTURES.get(platform.machine())
    if not arch:
        raise SystemExit(f"Unknown CPU architecture {platform.machine()}")
    homepage = package.get("homepage") or package.get("repository", "")
    mtime = int(os.environ.get("SOURCE_DATE_EPOCH", time.time()))

    files = {}
    for asset in meta["assets"]:
        source = binary if asset["source"] == f"target/release/{name}" else ROOT / asset["source"]
        files[asset["dest"].lstrip("/")] = (source.read_bytes(), int(asset["mode"], 8))
    holder = meta.get("copyright") or package["authors"][0].split(" <")[0]
    copyright_text = copyright_file(name, homepage, package["license"], holder)
    files[f"usr/share/doc/{name}/copyright"] = (copyright_text, 0o644)

    folders = {"."}
    for path in files:
        parent = Path(path).parent
        while str(parent) != ".":
            folders.add(str(parent))
            parent = parent.parent
    data_entries = [("./" if f == "." else f"./{f}/", None, 0o755) for f in folders]
    data_entries += [(f"./{path}", data, mode) for path, (data, mode) in files.items()]

    installed_kib = math.ceil(sum(len(data) for data, _ in files.values()) / 1024)
    description = "\n".join(
        " " + line for line in textwrap.wrap(meta.get("extended-description", ""), 76)
    )
    control = (
        f"Package: {name}\n"
        f"Version: {version}\n"
        f"Architecture: {arch}\n"
        f"Maintainer: {package['authors'][0]}\n"
        f"Installed-Size: {installed_kib}\n"
        f"Depends: {depends(binary)}\n"
        f"Section: {meta.get('section', 'utils')}\n"
        f"Priority: {meta.get('priority', 'optional')}\n"
        f"Homepage: {homepage}\n"
        f"Description: {package['description']}\n"
        f"{description}\n"
    ).encode()
    md5sums = "".join(
        f"{hashlib.md5(data).hexdigest()}  {path}\n" for path, (data, _) in sorted(files.items())
    ).encode()
    control_tar = tar([("./", None, 0o755), ("./control", control, 0o644), ("./md5sums", md5sums, 0o644)], mtime)
    data_tar = tar(data_entries, mtime)

    output.mkdir(parents=True, exist_ok=True)
    deb = output / f"{name}_{version}_{arch}.deb"
    # Member order is fixed by the format: debian-binary, then control, then data.
    deb.write_bytes(ar([("debian-binary", b"2.0\n"), ("control.tar.xz", control_tar), ("data.tar.xz", data_tar)], mtime))
    print(deb)


if __name__ == "__main__":
    main()
