#!/bin/sh
# SPDX-License-Identifier: MPL-2.0
set -eu
echo 'Floating-point formatting through BusyBox and musl:'
printf 'pi = %.6f; fraction = %.3f\n' 3.14159265 0.125
value=$(printf '%.1f' 1.5)
[ "$value" = 1.5 ]
echo 'RV32 ILP32D userspace formatted 1.5 successfully.'
