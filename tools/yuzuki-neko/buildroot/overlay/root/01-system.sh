#!/bin/sh
# SPDX-License-Identifier: MPL-2.0
set -eu
echo 'Kernel and architecture:'
uname -a
echo 'Memory reported by procfs:'
cat /proc/meminfo
echo 'Uptime (seconds):'
cat /proc/uptime
echo 'Mounted filesystems:'
cat /proc/mounts
