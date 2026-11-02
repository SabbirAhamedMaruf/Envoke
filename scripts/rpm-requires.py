#!/usr/bin/env python3
"""Emit RPM requires metadata from this project's native ELF binary.

Read ELF version needs directly: ldd -v can annotate optional versions with
[WEAK], which cargo-generate-rpm 0.21.0's built-in scanner misparses as part
of the dependency name. Weak ELF versions are not hard RPM requirements.
"""
import json
import os
from pathlib import Path
import re
import subprocess
import sys


def readelf(binary, option):
    return subprocess.check_output(
        ["readelf", "--wide", option, str(binary)],
        text=True, env={**os.environ, "LC_ALL": "C"},
    )


def requirements(binary):
    with binary.open("rb") as file:
        header = file.read(5)
    if header[:4] != b"\x7fELF" or header[4] not in (1, 2):
        raise ValueError("Expected a native ELF executable")
    marker = "(64bit)" if header[4] == 2 else ""
    libraries = re.findall(r"\(NEEDED\).*?\[([^\]]+)\]", readelf(binary, "--dynamic"))
    required = {f"{lib}(){marker}" for lib in libraries}
    version_info = readelf(binary, "--version-info")
    current_library = None
    in_needs = False
    for line in version_info.splitlines():
        if line.startswith("Version needs section"):
            in_needs = True
        elif line.startswith("Version ") and "section" in line:
            in_needs = False
        if not in_needs:
            continue
        library = re.search(r"File:\s+(\S+)", line)
        if library:
            current_library = library[1]
        version = re.search(r"Name:\s+(\S+)\s+Flags:\s+(\S+)", line)
        if version and version[2] != "WEAK":
            if current_library is None:
                raise ValueError("ELF version requirement without a library")
            required.add(f"{current_library}({version[1]}){marker}")
    sections = readelf(binary, "--section-headers")
    if re.search(r"\s\.gnu\.hash\s", sections) and not re.search(r"\s\.hash\s", sections):
        required.add("rtld(GNU_HASH)")
    return sorted(required)


if __name__ == "__main__":
    deps = requirements(Path(sys.argv[1]))
    print("[requires]")
    for dependency in deps:
        print(f'{json.dumps(dependency)} = "*"')
