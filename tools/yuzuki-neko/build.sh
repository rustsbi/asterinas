#!/bin/bash
# SPDX-License-Identifier: MPL-2.0

set -euo pipefail

arch=riscv32
if [[ ${1:-} == --arch ]]; then
    arch=${2:-}
    if [[ $arch != riscv32 && $arch != riscv64 ]]; then
        echo "--arch must be riscv32 or riscv64" >&2
        exit 1
    fi
    shift 2
fi
args=()
if [[ $# -gt 0 ]]; then
    if [[ $# -ne 2 || ! $2 =~ ^(0x[0-9a-fA-F]+|[0-9]+)$ ]]; then
        echo "Usage: bash tools/yuzuki-neko/build.sh [--arch riscv32|riscv64] [INITRAMFS LOAD_ADDRESS]" >&2
        exit 1
    fi
    initramfs=$(realpath "$1")
    start=$(( $2 ))
    end=$(( start + $(stat -c %s "$initramfs") ))
    args+=(--initramfs "$initramfs" --initramfs-address "$start")
fi

# Run in the Asterinas kernel-dev container. No board access or flashing.
repo=$(cd "$(dirname "$0")/../.." && pwd)
scheme=yuzuki-neko
if [[ $arch == riscv32 ]]; then
    scheme=yuzuki-neko-rv32
fi
out="$repo/target/$scheme"
mkdir -p "$out"
: "${VDSO_LIBRARY_DIR:?Set VDSO_LIBRARY_DIR as in the kernel-dev container}"
if [[ $arch == riscv32 ]]; then
    sh "$repo/tools/rv32-vdso/build.sh" "$VDSO_LIBRARY_DIR"
fi
export OSTD_TASK_STACK_SIZE_IN_PAGES=32
export CARGO_TARGET_DIR="$repo/target"

cd "$repo/kernel"
cargo osdk build --target-arch "$arch" --scheme "$scheme" \
    --config 'profile.release-lto.opt-level="z"'

elf="$repo/target/${arch}imac-unknown-none-elf/release-lto/asterinas-osdk-bin"
dtc -I dts -O dtb -o "$out/yuzuki-neko.dtb" "$repo/tools/yuzuki-neko/yuzuki-neko.dts"
if [[ $arch == riscv32 ]]; then
    fdtput -t s "$out/yuzuki-neko.dtb" / model "Yuzuki Neko (Asterinas RV32)"
    fdtput -t s "$out/yuzuki-neko.dtb" /cpus/cpu@0 mmu-type "riscv,sv32"
    fdtput -t s "$out/yuzuki-neko.dtb" /cpus/cpu@0 riscv,isa-base "rv32i"
    # The shared vendor DT also advertises extensions that only apply to RV64 PTEs.
    read -r -a extensions <<< "$(fdtget -t s "$out/yuzuki-neko.dtb" /cpus/cpu@0 riscv,isa-extensions)"
    rv32_extensions=()
    for extension in "${extensions[@]}"; do
        if [[ $extension != svpbmt && $extension != svnapot ]]; then
            rv32_extensions+=("$extension")
        fi
    done
    fdtput -t s "$out/yuzuki-neko.dtb" /cpus/cpu@0 riscv,isa-extensions "${rv32_extensions[@]}"
fi

if [[ $# -gt 0 ]]; then
    fdtput -t x "$out/yuzuki-neko.dtb" /chosen linux,initrd-start 0 "$(printf '%x' "$start")"
    fdtput -t x "$out/yuzuki-neko.dtb" /chosen linux,initrd-end 0 "$(printf '%x' "$end")"
fi

python3 "$repo/tools/yuzuki-neko/check_image.py" "$elf" \
    --arch "$arch" --dtb "$out/yuzuki-neko.dtb" "${args[@]}" --output-image "$out/Image"
cp "$elf" "$out/kernel.elf"
echo "Output directory: $out"
