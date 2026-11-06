#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Regression tests for ELF layout checking and RV64 raw-image relocation."""

import argparse
import contextlib
import io
from pathlib import Path
import tempfile
import unittest

from check_image import ELF_HEADERS, PROGRAM_HEADERS, check, load_elf


class ImageLayoutTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)

    def elf(self, bits, base, overlapping=False):
        header = ELF_HEADERS[bits]
        program_header = PROGRAM_HEADERS[bits]
        ident = b"\x7fELF" + bytes([1 if bits == 32 else 2, 1, 1]) + bytes(9)
        data = bytearray(0x2004)
        data[:header.size] = header.pack(
            ident, 2, 243, 1, base, header.size, 0, 0,
            header.size, program_header.size, 2, 0, 0, 0,
        )
        for index, relative in enumerate([0, 0x1000 if overlapping else 0x3000]):
            address = base + relative
            offset = 0x1000 * (index + 1)
            if bits == 32:
                values = (1, offset, address, address, 4, 0x2000, 5, 0x1000)
            else:
                values = (1, 5, offset, address, address, 4, 0x2000, 0x1000)
            program_header.pack_into(data, header.size + index * program_header.size, *values)
            data[offset:offset + 4] = b"BOOT" if index == 0 else b"DATA"
        path = self.root / f"kernel{bits}.elf"
        path.write_bytes(data)
        return path

    def args(self, elf, **overrides):
        values = dict(elf=elf, arch=None, kernel_address=0x40000000,
                      dtb=None, sbi=None, initramfs=None, initramfs_address=None,
                      min_free=0, output_image=self.root / "Image")
        values.update(overrides)
        return argparse.Namespace(**values)

    def run_check(self, args):
        with contextlib.redirect_stdout(io.StringIO()):
            check(args)

    def test_rv32_image_preserves_file_data_and_zeroes_bss_and_gaps(self):
        args = self.args(self.elf(32, 0x40000000))
        self.run_check(args)
        image = args.output_image.read_bytes()
        self.assertEqual(len(image), 0x5000)
        self.assertEqual(image[:4], b"BOOT")
        self.assertEqual(image[4:0x3000], bytes(0x3000 - 4))
        self.assertEqual(image[0x3000:0x3004], b"DATA")
        self.assertEqual(image[0x3004:], bytes(0x2000 - 4))

    def test_rv64_relocates_raw_layout_without_modifying_elf(self):
        elf = self.elf(64, 0x80200000)
        original = elf.read_bytes()
        segments, entry, bits = load_elf(elf, 0x40000000)
        self.assertEqual((segments[0][0], segments[1][0], entry, bits),
                         (0x40000000, 0x40003000, 0x40000000, 64))
        self.run_check(self.args(elf, arch="riscv64"))
        self.assertEqual(elf.read_bytes(), original)
        self.assertEqual((self.root / "Image").read_bytes()[:4], b"BOOT")

    def test_rv32_does_not_silently_relocate_linked_addresses(self):
        with self.assertRaisesRegex(ValueError, "exceeds 16 MiB PSRAM"):
            load_elf(self.elf(32, 0x80400000), 0x40000000)

    def test_overlapping_segments_are_rejected_before_relocation(self):
        with self.assertRaisesRegex(ValueError, "overlapping PT_LOAD"):
            load_elf(self.elf(64, 0x80200000, overlapping=True), 0x40000000)

    def test_unaligned_runtime_load_address_is_rejected(self):
        args = self.args(self.elf(64, 0x80200000), kernel_address=0x40001000)
        with self.assertRaisesRegex(ValueError, "must be 0x200000-aligned"):
            self.run_check(args)

    def test_initramfs_overlap_is_rejected(self):
        archive = self.root / "initramfs.cpio"
        archive.write_bytes(b"initramfs")
        args = self.args(self.elf(32, 0x40000000),
                         initramfs=archive, initramfs_address=0x40001000)
        with self.assertRaisesRegex(ValueError, "overlaps"):
            self.run_check(args)

    def test_wrong_architecture_is_rejected(self):
        args = self.args(self.elf(32, 0x40000000), arch="riscv64")
        with self.assertRaisesRegex(ValueError, "does not match"):
            self.run_check(args)


if __name__ == "__main__":
    unittest.main()
