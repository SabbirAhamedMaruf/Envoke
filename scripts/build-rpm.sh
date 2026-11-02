#!/usr/bin/env bash
# Build a native, locally installable RPM. No root privileges or rpmbuild needed.
set -euo pipefail

project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd -- "$project_root"

for tool in cargo rustc python3 rpm readelf sha256sum; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        printf 'Missing required tool: %s\n' "$tool" >&2
        exit 1
    fi
done

packager_version=0.21.0
packager="$project_root/target/rpm-tools/bin/cargo-generate-rpm"
if [[ ! -x "$packager" ]] || [[ "$("$packager" --version)" != "cargo-generate-rpm $packager_version" ]]; then
    cargo install cargo-generate-rpm --version "$packager_version" --locked \
        --root "$project_root/target/rpm-tools"
fi

# Pin the native target and output directory even if Cargo has cross-build defaults.
host_target="$(rustc -vV | awk '/^host:/ { print $2 }')"
package_name="$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["package"]["name"])')"
package_version="$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["package"]["version"])')"
package_arch="$(rpm --eval '%{_arch}')"
package_dist="$(rpm --eval '%{?dist}')"
package_release="${RPM_RELEASE:-1}${package_dist}"
if [[ ! "$package_release" =~ ^[A-Za-z0-9._+]+$ ]]; then
    printf 'Invalid RPM release: %s\n' "$package_release" >&2
    exit 1
fi

cargo build --release --locked --target "$host_target" --target-dir "$project_root/target"
python3 scripts/rpm-requires.py "$project_root/target/$host_target/release/$package_name" \
    > "$project_root/target/rpm-requires.toml"
mkdir -p -- "$project_root/dist"
package_file="$project_root/dist/$package_name-$package_version-$package_release.$package_arch.rpm"
"$packager" --target "$host_target" --target-dir "$project_root/target" \
    --arch "$package_arch" --auto-req disabled \
    --metadata-overwrite "$project_root/target/rpm-requires.toml" \
    --set-metadata "release=\"$package_release\"" --output "$package_file"

rpm -K "$package_file"
rpm -qp --queryformat 'Built: %{NAME}-%{VERSION}-%{RELEASE}.%{ARCH}\n' "$package_file"
(
    cd -- "$project_root/dist"
    sha256sum -- "$(basename -- "$package_file")" > "$(basename -- "$package_file").sha256"
)
printf '\nRPM: %s\nInstall manually: sudo dnf install "%s"\n' "$package_file" "$package_file"
