#!/bin/sh
# SPDX-License-Identifier: MPL-2.0
set -eu
cd /root
for demo in 01-system.sh 02-files.sh 03-pipes.sh 04-process.sh 05-signals.sh \
            06-float.sh memory-demo threads-demo ipc-demo; do
    printf '\n=== %s ===\n' "$demo"
    if ! "./$demo"; then
        echo "Example failed: $demo" >&2
        exit 1
    fi
done
printf '\nAll 9 Asterinas examples completed.\n'
