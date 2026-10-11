#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Check Yuzuki Neko's PSRAM layout and optionally emit a zero-filled raw image."""

import argparse
from pathlib import Path
import struct
import sys

RAM_START, RAM_END = 0x40000000, 0x41000000
DTB_START, SBI_START = 0x40F40000, 0x40F80000
PAGE_SIZE = 4096
METADATA_SIZE = (RAM_END - RAM_START) // PAGE_SIZE * 64
# Sv32 preallocates a child table for each shared kernel root entry.
SV32_KERNEL_PAGE_TABLE_SIZE = 768 * PAGE_SIZE
ELF_HEADERS = {32: struct.Struct("<16sHHIIIIIHHHHHH"),
               64: struct.Struct("<16sHHIQQQIHHHHHH")}
PROGRAM_HEADERS = {32: struct.Struct("<IIIIIIII"),
                   64: struct.Struct("<IIQQQQQQ")}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def load_elf(path):
    data = path.read_bytes()
    require(len(data) >= 16, "truncated ELF identification")
    require(data[:4] == b"\x7fELF" and data[4] in (1, 2) and data[5:7] == b"\x01\x01",
            "expected ELF32 or ELF64 little-endian")
    bits = 32 if data[4] == 1 else 64
    elf_header, program_header = ELF_HEADERS[bits], PROGRAM_HEADERS[bits]
    require(len(data) >= elf_header.size, "truncated ELF header")
    ident, kind, machine, version, entry, phoff, _, _, ehsize, phsize, count, *_ = (
        elf_header.unpack_from(data)
    )
    require(kind == 2 and machine == 243 and version == 1,
            "expected an executable RISC-V ELF")
    require(ehsize == elf_header.size and phsize == program_header.size,
            "invalid ELF or program header size")
    require(0 < count < 0xFFFF and phoff >= ehsize
            and phoff + count * phsize <= len(data), "invalid program header table")
    segments = []
    entry_addresses = []
    for index in range(count):
        header = program_header.unpack_from(data, phoff + index * phsize)
        if bits == 32:
            kind, offset, vaddr, paddr, filesz, memsz, flags, align = header
        else:
            kind, flags, offset, vaddr, paddr, filesz, memsz, align = header
        if kind != 1:
            continue
        require(filesz <= memsz and offset + filesz <= len(data),
                f"PT_LOAD {index}: truncated data or filesz exceeds memsz")
        require(vaddr + memsz <= 1 << bits and paddr + memsz <= 1 << bits,
                f"PT_LOAD {index}: address overflow")
        require(align in (0, 1) or (align & (align - 1) == 0
                and vaddr % align == offset % align),
                f"PT_LOAD {index}: invalid alignment")
        if not memsz:
            continue
        require(RAM_START <= paddr < paddr + memsz <= RAM_END,
                f"PT_LOAD {index}: {paddr:#x}..{paddr + memsz:#x} exceeds 16 MiB PSRAM")
        segments.append((paddr, memsz, data[offset:offset + filesz]))
        if flags & 1 and vaddr <= entry < vaddr + filesz:
            entry_addresses.append(paddr + entry - vaddr)
    require(segments, "ELF has no nonempty PT_LOAD segments")
    require(len(entry_addresses) == 1, "ELF entry must belong to one file-backed executable segment")
    segments.sort(key=lambda segment: segment[0])
    for left, right in zip(segments, segments[1:]):
        require(left[0] + left[1] <= right[0], "overlapping PT_LOAD physical segments")
    return segments, entry_addresses[0], bits


def file_size(path):
    require(path.is_file(), f"not a file: {path}")
    size = path.stat().st_size
    require(size > 0, f"empty file: {path}")
    return size


def check(args):
    segments, entry, bits = load_elf(args.elf)
    require(args.arch is None or args.arch == f"riscv{bits}",
            f"ELF{bits} does not match --arch {args.arch}")
    start = segments[0][0]
    end = max(address + size for address, size, _ in segments)
    require(start == args.kernel_address and start % PAGE_SIZE == 0,
            f"kernel starts at {start:#x}; expected page-aligned {args.kernel_address:#x}")
    require(entry == start, "physical ELF entry must match the raw image load address")
    # OSTD reserves the full kernel span, including BSS and gaps between segments.
    kernel_end = (end + PAGE_SIZE - 1) // PAGE_SIZE * PAGE_SIZE
    regions = [("kernel", start, kernel_end),
               ("DTB reservation", DTB_START, SBI_START),
               ("SBI reservation", SBI_START, RAM_END)]
    for path, capacity, name in [(args.dtb, SBI_START - DTB_START, "DTB"),
                                 (args.sbi, RAM_END - SBI_START, "SBI")]:
        if path is not None:
            require(file_size(path) <= capacity, f"{name} exceeds its {capacity} byte reservation")
    require((args.initramfs is None) == (args.initramfs_address is None),
            "--initramfs and --initramfs-address must be provided together")
    if args.initramfs is not None:
        address = args.initramfs_address
        require(address % PAGE_SIZE == 0, "initramfs address must be page-aligned")
        size = (file_size(args.initramfs) + PAGE_SIZE - 1) // PAGE_SIZE * PAGE_SIZE
        regions.append(("initramfs", address, address + size))
    for name, lower, upper in regions:
        require(RAM_START <= lower < upper <= RAM_END,
                f"{name}: {lower:#x}..{upper:#x} exceeds 16 MiB PSRAM")
    regions.sort(key=lambda region: region[1])
    for left, right in zip(regions, regions[1:]):
        require(left[2] <= right[1], f"{left[0]} overlaps {right[0]}")
    free = RAM_END - RAM_START - sum(upper - lower for _, lower, upper in regions)
    page_table_size = SV32_KERNEL_PAGE_TABLE_SIZE if bits == 32 else 0
    free -= METADATA_SIZE + page_table_size
    require(args.min_free >= 0, "--min-free must not be negative")
    require(free >= args.min_free,
            f"runtime budget is {free} bytes after reserving {METADATA_SIZE} bytes of "
            f"frame metadata and {page_table_size} bytes of shared kernel page tables; "
            f"need at least {args.min_free} bytes")
    if args.output_image is not None:
        inputs = [args.elf, args.dtb, args.sbi, args.initramfs]
        require(all(path is None or path.resolve() != args.output_image.resolve() for path in inputs),
                "output image must not overwrite an input file")
        # A raw loader does not clear ELF BSS. Include zeroed BSS and internal gaps.
        image = bytearray(end - start)
        for address, _, contents in segments:
            offset = address - start
            image[offset:offset + len(contents)] = contents
        args.output_image.write_bytes(image)
        print(f"Raw image: {args.output_image} ({len(image)} bytes), load at {start:#x}")
    for name, lower, upper in regions:
        print(f"{name}: {lower:#010x}..{upper:#010x} ({upper - lower} bytes)")
    print(f"RV{bits} physical entry: {entry:#x}; runtime budget after metadata and page tables: {free} bytes")
    print("Layout check passed; this does not verify boot or sufficient runtime memory.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("elf", type=Path, help="linked kernel ELF")
    parser.add_argument("--arch", choices=("riscv32", "riscv64"), help="expected kernel architecture")
    parser.add_argument("--kernel-address", type=lambda value: int(value, 0), default=RAM_START)
    parser.add_argument("--dtb", type=Path, help="DTB binary loaded at 0x40f40000")
    parser.add_argument("--sbi", type=Path, help="SBI raw binary loaded at 0x40f80000")
    parser.add_argument("--initramfs", type=Path)
    parser.add_argument("--initramfs-address", type=lambda value: int(value, 0))
    parser.add_argument("--min-free", type=lambda value: int(value, 0), default=0x200000,
                        help="minimum runtime budget in bytes after metadata (default: 2 MiB)")
    parser.add_argument("--output-image", type=Path, help="write raw kernel with zeroed BSS and gaps")
    args = parser.parse_args()
    try:
        check(args)
    except (OSError, ValueError, struct.error) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
