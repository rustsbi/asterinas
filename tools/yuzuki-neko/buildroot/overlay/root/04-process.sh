#!/bin/sh
# SPDX-License-Identifier: MPL-2.0
set -eu
echo "Parent PID: $$"
sleep 1 &
child=$!
echo "Started child PID: $child; parent waits for it."
wait "$child"
echo 'Child completed.'
(exit 7) &
child=$!
if wait "$child"; then
    echo 'Unexpected child status' >&2
    exit 1
else
    status=$?
    echo "Collected child exit status: $status"
    [ "$status" -eq 7 ]
fi
