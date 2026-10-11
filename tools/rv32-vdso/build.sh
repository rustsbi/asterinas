#!/bin/sh
# SPDX-License-Identifier: MPL-2.0
set -eu

# Usage: sh tools/rv32-vdso/build.sh "$VDSO_LIBRARY_DIR"
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
out_dir=${1:?Pass the VDSO_LIBRARY_DIR output directory}
cross_compile=${CROSS_COMPILE:-riscv64-linux-gnu-}
mkdir -p "$out_dir"

"${cross_compile}gcc" -march=rv32imac -mabi=ilp32 -mno-relax \
    -nostdlib -nostartfiles -shared -fPIC \
    -Wl,--build-id=none -Wl,--no-undefined -Wl,--hash-style=both \
    -Wl,-Bsymbolic -Wl,-z,max-page-size=4096 \
    -Wl,-soname=linux-vdso.so.1 -Wl,-T,"$script_dir/vdso.ld" \
    "$script_dir/vdso.S" -o "$out_dir/vdso_riscv32.so"

echo "Built $out_dir/vdso_riscv32.so"
