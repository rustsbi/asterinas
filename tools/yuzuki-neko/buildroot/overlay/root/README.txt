Asterinas examples for Yuzuki Neko
================================

Run everything: ./run-demos.sh
Read this guide: cat /root/README.txt

Command             Demonstrates
./01-system.sh      uname, procfs memory/uptime, mounted filesystems
./02-files.sh       create/read/copy/rename/delete files in /tmp
./03-pipes.sh       pipelines, sorting, filtering and line counting
./04-process.sh     background processes, wait and child exit status
./05-signals.sh     SIGUSR1 delivery and a shell signal handler
./06-float.sh       RV32 ILP32D floating-point formatting via musl
./memory-demo       mmap, touching 16 pages, mprotect and munmap
./threads-demo      two pthreads, a mutex-protected counter and joins
./ipc-demo          fork, a pipe, private memory and waitpid

Each example runs on its own and exits. The runner stops on an error.
C source is available in /root/src; these binaries use static musl.
The native programs exercise Asterinas's Linux-compatible userspace ABI.
The board has one CPU: multiple threads demonstrate scheduling, not SMP.
Examples allocate modest amounts of memory and finish within seconds.

The root filesystem is in RAM. Changes disappear on reboot.
File examples clean up only their own temporary files.
These examples do not require network access or change flash contents.

Power commands (run manually, excluded from run-demos.sh):
  reboot     SBI ColdReboot through RustSBI's F101 watchdog
  poweroff   SBI Shutdown request, then CPU halt fallback on this board
  halt       same CPU halt behavior
USB power remains on after poweroff/halt; press reset to boot again.

Serial: 115200, 8N1, no flow control. Avoid fast multiline pastes.
