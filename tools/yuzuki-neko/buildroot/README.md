# Minimal Buildroot userspace for Asterinas on Neko

This external tree builds a small, uncompressed `newc` initramfs with BusyBox,
musl, an interactive `ash` shell, and basic file utilities. `/init` mounts
devtmpfs, procfs, and sysfs, starts a shell on `ttyS0`, and restarts it after exit.
The filesystem lives in RAM; changes disappear after reboot.

`reboot`, `poweroff`, and `halt` are enabled. They signal PID 1, whose traps
invoke the corresponding BusyBox applet with `-f` to reach the kernel's existing
reboot syscall and SBI reset handler. `reboot` uses RustSBI's F101 watchdog.
The current Neko firmware has no power-cut backend: `poweroff` and `halt`
stop the CPU through OSTD's fallback, but USB power remains connected.

## Examples in /root

Run `./run-demos.sh` from the initial shell to demonstrate system information,
RAM-backed files, pipelines, process waits, signals, floating-point formatting,
anonymous memory mappings, pthread mutexes, and parent/child IPC.
Each example can also run individually; `cat /root/README.txt` lists commands.
The three static C programs are built by `post-build.sh` with the same toolchain
as BusyBox, and their sources are installed in `/root/src`.
They use small bounded allocations suitable for the board's 16 MiB RAM.
The runner does not reboot or shut down the board.

## Build

From the Asterinas repository root, in a Linux build environment with the
[Buildroot host dependencies](https://buildroot.org/downloads/manual/manual.html#requirement-mandatory):

```sh
bash tools/yuzuki-neko/build-initramfs.sh
```

The script downloads the pinned Buildroot 2025.02.6 release and checks its
SHA-256. Buildroot downloads and verifies the standard Bootlin
`riscv32-ilp32d--musl--stable-2024.05-1` toolchain and BusyBox 1.37.0.
The toolchain contains GCC 13.3.0 and musl 1.2.5. Set `JOBS` to change the
parallelism, or `NEKO_BUILDROOT_OUTPUT` to select a separate output directory.

The result is `target/neko-buildroot/output/images/rootfs.cpio`. The script
rejects archives larger than the existing 1 MiB Neko initramfs slot.
`configs/neko_defconfig` contains the Buildroot configuration and
`busybox.config` contains the complete minimal BusyBox configuration.

Userspace uses RV32GC/ILP32D, supported by Neko's C907 F/D extensions and
Asterinas's existing floating-point context support. The kernel remains
`riscv32imac-unknown-none-elf`. This image requires a CPU with F and D.

Use the standard musl toolchain selected by the defconfig. The previously used
Xuantie V3.2.0 musl32 toolchain passes a vendor `sa_restorer` field to
`rt_sigaction`, which does not match the upstream Linux RISC-V signal ABI.
BusyBox's self-execution path is explicitly `/bin/busybox`: the current
Asterinas `/proc/self/exe` points to the script when executing `/init` through
its shebang, so the BusyBox default would recursively execute the init script.

## Kernel

Build the kernel as described in the [Neko port documentation](../README.md):

```sh
bash tools/yuzuki-neko/build.sh --arch riscv32 \
    target/neko-buildroot/output/images/rootfs.cpio 0x40500000
```

## Serial use

Use 115200 baud, 8 data bits, no parity, 1 stop bit, and no flow control.
The original board validation used COM18; select the serial port for your host. At `neko:~#`, try `ls`, `cat /proc/meminfo`,
`echo hello > /tmp/hello`, or `busybox` to list applets. Type `help` for shell
builtins. There are no networking services or package manager in this image.

The existing Neko UART burst-input limitation remains: host-driven command
input uses 20 ms between characters. Avoid pasting whole command blocks.
The previous NOR rootfs is preserved, but Asterinas has no native SPI NOR
filesystem driver; this shell uses the RAM root filesystem only.
