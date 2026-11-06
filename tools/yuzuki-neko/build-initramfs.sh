#!/bin/bash
# SPDX-License-Identifier: MPL-2.0
set -euo pipefail

repo=$(cd "$(dirname "$0")/../.." && pwd)
external="$repo/tools/yuzuki-neko/buildroot"
out=${NEKO_BUILDROOT_OUTPUT:-"$repo/target/neko-buildroot"}
mkdir -p "$out/downloads"
out=$(realpath "$out")

version=2025.02.6
archive="$out/downloads/buildroot-$version.tar.xz"
if [[ ! -f $archive ]]; then
    curl --fail --location --retry 3 "https://buildroot.org/downloads/buildroot-$version.tar.xz" -o "$archive.part"
    mv "$archive.part" "$archive"
fi
echo "a49881ff3042f458e8dbff8173bf0e5c53a84e426ec58f7343eda5e6fec76d89  $archive" | sha256sum --check
source="$out/buildroot-$version"
if [[ ! -d $source ]]; then
    tar -xf "$archive" -C "$out"
fi

make -C "$source" O="$out/output" BR2_EXTERNAL="$external" neko_defconfig
make -C "$source" O="$out/output" BR2_EXTERNAL="$external" -j"${JOBS:-16}"
image="$out/output/images/rootfs.cpio"
test "$(stat -c %s "$image")" -le 1048576 || {
    echo 'initramfs exceeds the 1 MiB NOR slot' >&2
    exit 1
}
echo "Initramfs: $image ($(stat -c %s "$image") bytes)"
