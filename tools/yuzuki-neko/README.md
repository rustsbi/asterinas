# Yuzuki Neko (Allwinner F101S3)

This port targets the 16 MiB Yuzuki Neko with an RV32/Sv32 kernel by
default. RV64/Sv39 remains available. It uses SBI console/timer/reset,
T-Head PLIC interrupts and interrupt-driven UART1 (`ttyS0`). Firmware must
initialize PSRAM, UART1 clocks and PB0/PB1 pinmux, cache/PMP state and
PLIC delegation before entering the kernel in S-mode.

F101 starts in RV32. The kernel, SBI firmware, DTB and userspace must use
the same ISA width. RV64 additionally requires the
[SyterKit reset-based mode switch](https://github.com/YuzukiHD/SyterKit/blob/505043246dee4de672d1ed408909c227271601df/boards/yuzukineko/arch_boot/payloads/arch_boot.S).
The hardware description follows the
[F101 Linux device trees](https://github.com/YuzukiHD/linux-mainline/tree/735f2ca14b7b916b17fa111a941a9a79d0c12e61/arch/riscv/boot/dts/allwinner).

## Build

Use the project's `asterinas/kernel-dev:0.18.1-20260901` container or the
pinned Rust toolchain with `rust-src`, `dtc`, `fdtget`, `fdtput`, Python 3
and `VDSO_LIBRARY_DIR` configured as in the
[kernel-dev Dockerfile](../docker/kernel-dev/Dockerfile).
RV32 also needs `riscv64-linux-gnu-gcc` with RV32 code generation;
`CROSS_COMPILE` can select another compatible GNU toolchain prefix.

From the repository root:

```sh
# Rebuild OSDK with this checkout's targets and linker templates.
OSDK_LOCAL_DEV=1 cargo install cargo-osdk --path osdk --locked
bash tools/yuzuki-neko/build.sh                 # RV32/Sv32
bash tools/yuzuki-neko/build.sh --arch riscv64  # RV64/Sv39
```

Both builds use size optimization, LTO and 128 KiB kernel task stacks.
The output directory contains `Image`, `yuzuki-neko.dtb` and `kernel.elf`:

| Architecture | Rust target | Neko OSDK scheme / output directory |
| --- | --- | --- |
| RV32 | `riscv32imac-unknown-none-elf` | `yuzuki-neko-rv32` / `target/yuzuki-neko-rv32` |
| RV64 | `riscv64imac-unknown-none-elf` | `yuzuki-neko` / `target/yuzuki-neko` |

The RV32 build automatically runs [the vDSO builder](../rv32-vdso/build.sh),
placing a real ELF32 `vdso_riscv32.so` in `VDSO_LIBRARY_DIR`. It provides
signal return and syscall fallbacks, including the RV32 time64 ABI.
The generated RV32 DT selects `rv32i`/Sv32 and excludes the RV64-only
Svpbmt/Svnapot PTE extensions.

The raw Image includes zero-filled BSS and gaps between ELF segments.
An ordinary `objcopy -O binary` output may omit this required initialization.
To reach userspace, supply a small initramfs with a matching RV32 or RV64
executable `/init` and any interpreter/libraries it needs:

```sh
bash tools/yuzuki-neko/build.sh --arch riscv32 /absolute/path/initramfs.cpio 0x40d00000
```

The builder checks that address against the actual image sizes and writes
`linux,initrd-start/end` into the DTB. Without an initramfs there is no root
filesystem from which to run init. The normal development initramfs is too
large; budget for both a minimal archive and its unpacked filesystem.

## Memory and loader contract

| Item | Physical address / reservation |
| --- | --- |
| PSRAM | `0x40000000..0x41000000` (16 MiB) |
| Kernel Image / entry | `0x40000000` |
| DTB | `0x40f40000..0x40f80000` |
| Matching RV32 or RV64 SBI | `0x40f80000..0x41000000` |
| Initramfs | Explicit address passed to `build.sh` |

The SBI handoff is `a0 = 0`, `a1 = final DTB physical address`, S-mode,
MMU disabled. SBI DBCN and TIME are required; reset additionally requires
SRST. The kernel preserves DTB and firmware reservations. UART1 retains
the firmware-configured baud rate, normally 115200 8N1. The provided DT
contains the devices needed for boot.

RustSBI can emit legal `FDT_NOP` tokens that the current `fdt` parser misses.
The kernel removes these tokens in its own DTB copy, preserving the firmware
blob, reservation map and reserved-memory nodes. DTBs containing NOPs must
fit the 64 KiB normalization buffer; this limit does not apply to DTBs
without NOPs. The buffer always occupies 64 KiB of BSS and is included in
the kernel span checked by the image builder.

Sv32 reserves virtual addresses below `0x40000000` for userspace (1 GiB).
The kernel is linked at `0xf0000000`; its physical load address must be
4 MiB aligned. `check_image.py` handles ELF32 and ELF64, rejects overlapping
images, and budgets 256 KiB of frame metadata plus at least 2 MiB of runtime
allocations. RV32 additionally reserves 3 MiB for shared kernel page tables.
Passing this check does not establish sufficient memory for every workload.

Sv32 handles accessed/dirty (A/D) bits in software where needed. Kernel
PTEs start with both bits set; user page faults set A when the existing
mapping permits the access, and set D on permitted writes. Copy-on-write
permission handling remains unchanged. Early traps report their cause,
faulting instruction address and fault address to diagnose startup failures.

For RAM-only RV32 boot, use SyterKit's
[`usb-boot` application](https://github.com/luojia65/SyterKit/blob/3bf4f61fc7a1a40137a49482e1a225fef0b97a42/boards/yuzukineko/app_sram/usb-boot/main.c),
with `yuzukineko_rv32_usb_defconfig` and `apps-sram-y=usb-boot`.
Initialize PSRAM through the host's FEL tool, write the Image, DTB,
initramfs and RV32 `fw_jump.bin` to their addresses, then load the SPL at
`0x00020000` and execute its entry at `0x00020040`; it validates
the staged DTB and calls SBI at `0x40f80000` with hart 0 and that DTB.
Load the SPL last: FEL register-access and cache helper commands can use
`0x00020000` as scratch space and overwrite it before execution.
This path does not initialize PSRAM again, read NOR, or switch ISA width.
The referenced USB defconfig enables I-cache but leaves D-cache disabled;
the loader's cache configuration must be accounted for at SBI handoff.

SBI's entire runtime allocation, including stacks, heap and BSS, must fit
512 KiB. The existing Neko RV32 RustSBI jump firmware uses a 376 KiB
reservation ending at `0x40fde000`, with the next stage at `0x40000000`.
For RV64, the
[F101 OpenSBI build script](https://github.com/YuzukiHD/opensbi/blob/08cbbe0bf6d2aaf0e8dbef8c548f4befdf3b7a7e/build.sh)
supports `--arch rv64`, `FW_TEXT_START=0x40f80000` and
`FW_JUMP_ADDR=0x40000000`; check its runtime size separately.
The build tools do not access the board or create a NOR flash image.

## QEMU and validation

QEMU kernels use different physical addresses from Neko:

| Architecture | QEMU OSDK scheme | Default physical load address |
| --- | --- | --- |
| RV32/Sv32 | `riscv32` | `0x80400000` |
| RV64 | `riscv` | `0x80200000` |

Build from `kernel/` with `cargo osdk build --target-arch riscv32 --scheme riscv32`,
or `--target-arch riscv64 --scheme riscv`, with
`OSTD_TASK_STACK_SIZE_IN_PAGES=32`. A direct RV32 OSDK build also needs
`sh tools/rv32-vdso/build.sh "$VDSO_LIBRARY_DIR"` run from the repository root
first. QEMU needs matching RV32/RV64 SBI firmware. For an RV64 Sv39 build,
select `riscv_sv39_mode` and run with `-cpu rv64,sv48=off,sv57=off`: the
RV64 assembly probes Sv48 before Sv39. QEMU `virt` does not emulate F101
and cannot validate its UART wiring, PSRAM initialization, clock setup
or RV64 reset sequence.

The final RV32/Sv32 build passed 16 MiB QEMU 8.2.2 boots with both an
ordinary DTB and a DTB containing 447 injected NOP tokens. Both reached
userspace and passed anonymous-page read/write checks, three 250 ms sleeps
with monotonic-clock checks, two signal round trips and serial input/echo.
The final RV64/Sv39 build also passed the 16 MiB userspace, timer and serial
regression. An earlier RV64 run passed with the DTB in reserved tail RAM.

The first RAM-only RV32 attempt on Neko entered `riscv_boot`; the Image
read back from RAM matched its original SHA-256. Boot then failed to find
`/chosen` because of the NOP parser issue described above. After DTB and
software A/D fixes, the fourth and fifth attempts reached RV32 `/init`
and passed anonymous-page access, timer and signal-return checks.
The full automated board smoke test failed its serial check: continuous
UART input can lose characters or introduce control characters. Probes on
the running board echoed complete lines with 20 ms and 100 ms between
bytes, while 0 ms bursts failed. Burst-input loss remains a known limitation.

SD/MMC, SPI/NOR, USB, GPIO buses, display and audio drivers are not added.
The RTC uses the fallback clock starting at the Unix epoch.
