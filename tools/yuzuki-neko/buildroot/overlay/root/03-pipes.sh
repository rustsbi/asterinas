#!/bin/sh
# SPDX-License-Identifier: MPL-2.0
set -eu
echo 'Sort input and pass it through a pipe to grep:'
printf 'banana\napple\napricot\n' | sort | grep '^ap'
count=$(printf 'banana\napple\napricot\n' | sort | grep '^ap' | wc -l)
echo "Matching lines: $count"
[ "$count" -eq 2 ]
