#!/bin/sh
# SPDX-License-Identifier: MPL-2.0
set -eu
target=$1
chmod 0755 "$target/init"
mkdir -p "$target/proc" "$target/sys" "$target/tmp" "$target/root"
chmod 1777 "$target/tmp"
# Buildroot's generic skeleton links /init to init; the overlay supplies our script.
test -f "$target/init" && test ! -L "$target/init"

# Build the small static userspace examples with the same ABI as BusyBox.
examples=$(dirname "$0")/examples
mkdir -p "$target/root/src"
for name in memory-demo threads-demo ipc-demo; do
    "$HOST_DIR/bin/riscv32-linux-gcc" -std=c11 -Os -static -pthread \
        -Wall -Wextra -Werror -ffunction-sections -fdata-sections \
        -Wl,--gc-sections -s "$examples/$name.c" -o "$target/root/$name"
    cp "$examples/$name.c" "$target/root/src/"
done
chmod 0755 "$target/root/"*.sh
