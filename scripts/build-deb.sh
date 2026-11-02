#!/usr/bin/env bash
# Build a native .deb for Debian / Ubuntu. No root, dpkg or extra packaging tools needed.
set -euo pipefail

project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd -- "$project_root"

for tool in cargo rustc python3 readelf sha256sum; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        printf 'Missing required tool: %s\n' "$tool" >&2
        exit 1
    fi
done

# Pin the native target and output directory even if Cargo has cross-build defaults.
host_target="$(rustc -vV | awk '/^host:/ { print $2 }')"
package_name="$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["package"]["name"])')"

cargo build --release --locked --target "$host_target" --target-dir "$project_root/target"
package_file="$(python3 scripts/build-deb.py \
    "$project_root/target/$host_target/release/$package_name" "$project_root/dist")"
(
    cd -- "$project_root/dist"
    sha256sum -- "$(basename -- "$package_file")" > "$(basename -- "$package_file").sha256"
)
printf '\nDEB: %s\nInstall on Debian / Ubuntu: sudo apt install "./dist/%s"\n' \
    "$package_file" "$(basename -- "$package_file")"
