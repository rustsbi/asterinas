#!/bin/sh
# SPDX-License-Identifier: MPL-2.0
set -eu
received=0
trap 'received=1; echo "Handled SIGUSR1 in PID $$"' USR1
echo "Sending SIGUSR1 to this example (PID $$)."
kill -USR1 "$$"
[ "$received" -eq 1 ]
echo 'The handler returned and execution continued.'
