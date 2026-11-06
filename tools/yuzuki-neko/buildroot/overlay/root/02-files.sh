#!/bin/sh
# SPDX-License-Identifier: MPL-2.0
set -eu
work=/tmp/neko-files-$$
mkdir "$work"
trap 'rm -f "$work/hello.txt" "$work/copy.txt" "$work/renamed.txt"; rmdir "$work"' EXIT
printf 'Hello from Asterinas!\nThis file lives in RAM.\n' > "$work/hello.txt"
cp "$work/hello.txt" "$work/copy.txt"
mv "$work/copy.txt" "$work/renamed.txt"
echo 'Created, copied and renamed a file:'
cat "$work/renamed.txt"
ls -l "$work"
echo 'Temporary files will be removed when this example exits.'
