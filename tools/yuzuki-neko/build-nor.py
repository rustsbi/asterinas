#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Combine Neko's fixed SPI NOR slots without touching the old rootfs or data."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
import subprocess
import sys


def update_initrd(dtb, start, end, loglevel):
    """Update existing 64-bit /chosen initrd properties in a copy of the DTB."""
    data = bytearray(dtb)
    magic, total, offset, strings = struct.unpack_from('>4I', data)
    if magic != 0xD00DFEED or total != len(data):
        raise ValueError('invalid DTB header')
    values = {'linux,initrd-start': start, 'linux,initrd-end': end}
    found, nodes = set(), []
    while True:
        tag, = struct.unpack_from('>I', data, offset)
        offset += 4
        if tag == 1:
            end_name = data.index(0, offset)
            nodes.append(data[offset:end_name].decode())
            offset = (end_name + 4) & ~3
        elif tag == 2:
            nodes.pop()
        elif tag == 3:
            size, name_offset = struct.unpack_from('>II', data, offset)
            offset += 8
            name_start = strings + name_offset
            name = data[name_start:data.index(0, name_start)].decode()
            if nodes == ['', 'chosen'] and name in values:
                if size != 8 or name in found:
                    raise ValueError('invalid or duplicate initrd property')
                struct.pack_into('>Q', data, offset, values[name])
                found.add(name)
            if nodes == ['', 'chosen'] and name == 'bootargs':
                original = bytes(data[offset:offset + size])
                replacement, count = re.subn(rb'(?<!\S)loglevel=[0-8](?!\d)',
                                            f'loglevel={loglevel}'.encode(), original)
                if count != 1 or len(original) != len(replacement):
                    raise ValueError('DTB needs one single-digit loglevel boot argument')
                data[offset:offset + size] = replacement
            offset += (size + 3) & ~3
        elif tag == 4:
            continue
        elif tag == 9:
            break
        else:
            raise ValueError(f'invalid DTB token: {tag}')
    if found != set(values):
        raise ValueError('DTB needs linux,initrd-start/end; first build with an initramfs')
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('kernel', 'dtb', 'sbi', 'spl', 'initramfs', 'output-dir'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--loglevel', type=int, choices=range(9), default=3,
                        help='kernel console verbosity; default 3 keeps the interactive shell quiet')
    args = parser.parse_args()
    out = args.output_dir.resolve()
    out.mkdir(parents=True, exist_ok=True)
    inputs = [args.kernel, args.dtb, args.sbi, args.spl, args.initramfs]
    outputs = [out / name for name in ('Image', 'yuzuki-neko.dtb', 'asterinas-buildroot-nor.bin', 'manifest.json')]
    if any(path.resolve() in outputs for path in inputs):
        raise ValueError('output files must not overwrite inputs')
    initrd = args.initramfs.read_bytes()
    if not initrd.startswith(b'070701') or not 0 < len(initrd) <= 0x100000:
        raise ValueError('expected a newc initramfs no larger than the 1 MiB NOR slot')
    spl = args.spl.read_bytes()
    if spl[4:12] != b'eGON.BT0' or len(spl) % 8192 or not 0 < len(spl) <= 0x10000:
        raise ValueError('invalid eGON SPI NOR SPL size or header')
    jump, = struct.unpack_from('<I', spl)
    checksum, length = struct.unpack_from('<II', spl, 12)
    words = list(struct.unpack(f'<{len(spl) // 4}I', spl))
    words[3] = 0x5F0A6C39
    if jump != 0x0400006F or length != len(spl) or sum(words) & 0xFFFFFFFF != checksum:
        raise ValueError('SPL entry or additive checksum mismatch')
    dtb = out / 'yuzuki-neko.dtb'
    dtb.write_bytes(update_initrd(args.dtb.read_bytes(), 0x40500000,
                                  0x40500000 + len(initrd), args.loglevel))
    image = out / 'Image'
    subprocess.run([sys.executable, str(Path(__file__).with_name('check_image.py')),
                    str(args.kernel), '--arch', 'riscv32', '--dtb', str(dtb),
                    '--sbi', str(args.sbi), '--initramfs', str(args.initramfs),
                    '--initramfs-address', '0x40500000', '--output-image', str(image)], check=True)
    slots = [
        ('spl', args.spl, 0, 0x20000, 0x10000),
        ('dtb', dtb, 0x10000, 0x40F40000, 0x40000),
        ('rustsbi', args.sbi, 0x50000, 0x40F80000, 0x80000),
        ('kernel', image, 0xD0000, 0x40000000, 0x500000),
        ('initramfs', args.initramfs, 0x5D0000, 0x40500000, 0x100000),
    ]
    combined = bytearray(b'\xff') * 0x6D0000
    payloads = []
    for name, path, flash, ram, capacity in slots:
        data = path.read_bytes()
        if not 0 < len(data) <= capacity:
            raise ValueError(f'{name} exceeds its NOR slot')
        combined[flash:flash + len(data)] = data
        payloads.append(dict(name=name, file=path.name, bytes=len(data),
                             flash_offset=hex(flash), ram_address=hex(ram),
                             sha256=hashlib.sha256(data).hexdigest()))
    result = out / 'asterinas-buildroot-nor.bin'
    result.write_bytes(combined)
    manifest = dict(board='Yuzuki Neko F101S3', flash_bytes=0x1000000, flash_offset=0,
                    image_bytes=len(combined), image_sha256=hashlib.sha256(combined).hexdigest(),
                    preserved_range=['0x6d0000', '0x1000000'], payloads=payloads)
    (out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    print(f'NOR image: {result} ({len(combined)} bytes)')


if __name__ == '__main__':
    main()
