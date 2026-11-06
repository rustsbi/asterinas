// SPDX-License-Identifier: MPL-2.0

//! Normalizes firmware DTBs for the `fdt` parser before publishing them.

use core::cell::SyncUnsafeCell;

use fdt::Fdt;

const MAX_COMPACTED_SIZE: usize = 64 * 1024;
const FDT_BEGIN_NODE: u32 = 1;
const FDT_END_NODE: u32 = 2;
const FDT_PROP: u32 = 3;
const FDT_NOP: u32 = 4;
const FDT_END: u32 = 9;

#[repr(align(8))]
struct AlignedBlob([u8; MAX_COMPACTED_SIZE]);

static COMPACTED: SyncUnsafeCell<AlignedBlob> =
    SyncUnsafeCell::new(AlignedBlob([0; MAX_COMPACTED_SIZE]));

/// Reads the firmware DTB, removing structural NOPs when necessary.
///
/// # Safety
///
/// The pointer must reference a valid DTB that remains readable and immutable
/// for `'static`. This function must be called only once, by the BSP before
/// starting other harts or publishing the returned tree.
pub(super) unsafe fn from_ptr(ptr: *const u8) -> Fdt<'static> {
    // SAFETY: The caller guarantees the DTB's validity and lifetime.
    let original = unsafe { Fdt::from_ptr(ptr).expect("invalid firmware DTB") };
    // SAFETY: The valid DTB contains `total_size()` readable, immutable bytes.
    let blob = unsafe { core::slice::from_raw_parts(ptr, original.total_size()) };
    let start = read_u32(blob, 8) as usize;
    let size = read_u32(blob, 36) as usize;
    assert!(
        start >= 40 && start.is_multiple_of(4),
        "invalid DTB structure offset"
    );
    let end = start
        .checked_add(size)
        .expect("DTB structure size overflow");
    let structure = blob.get(start..end).expect("DTB structure exceeds blob");
    let mut nops = 0;
    visit_tokens(structure, |tag, _| nops += usize::from(tag == FDT_NOP));
    if nops == 0 {
        return original;
    }

    // `fdt` 0.1.5 misses legal NOPs between child nodes, including those
    // emitted by RustSBI. Copy rather than modifying the firmware's blob.
    assert!(
        blob.len() <= MAX_COMPACTED_SIZE,
        "DTB with NOP tokens exceeds 64 KiB normalization buffer"
    );
    // SAFETY: Only the BSP calls this function, once. No references to this
    // buffer exist yet, and it is never modified after the tree is returned.
    let output = unsafe { &mut (*COMPACTED.get()).0 };
    output[..blob.len()].copy_from_slice(blob);
    let mut written = 0;
    visit_tokens(structure, |tag, token| {
        if tag != FDT_NOP {
            output[start + written..start + written + token.len()].copy_from_slice(token);
            written += token.len();
        }
    });
    // Leave the strings, reservation map and total size untouched. The gap
    // after the shortened structure block is permitted by the DTB format.
    output[36..40].copy_from_slice(&(written as u32).to_be_bytes());
    Fdt::new(&output[..blob.len()]).expect("invalid normalized DTB")
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    let end = offset.checked_add(4).expect("DTB field offset overflow");
    u32::from_be_bytes(
        bytes
            .get(offset..end)
            .expect("truncated DTB field")
            .try_into()
            .unwrap(),
    )
}

fn padded_size(size: usize) -> usize {
    size.checked_add(3).expect("DTB token size overflow") & !3
}

/// Visits complete tokens, including their data and alignment padding.
fn visit_tokens(mut remaining: &[u8], mut visit: impl FnMut(u32, &[u8])) {
    let mut depth = 0usize;
    let mut saw_root = false;
    while !remaining.is_empty() {
        let tag = read_u32(remaining, 0);
        let size = match tag {
            FDT_BEGIN_NODE => {
                if depth == 0 {
                    assert!(!saw_root, "multiple DTB root nodes");
                    saw_root = true;
                }
                depth = depth.checked_add(1).expect("DTB node depth overflow");
                let name_len = remaining[4..]
                    .iter()
                    .position(|byte| *byte == 0)
                    .expect("unterminated DTB node name")
                    + 1;
                4usize
                    .checked_add(padded_size(name_len))
                    .expect("DTB node size overflow")
            }
            FDT_PROP => {
                assert!(depth != 0, "DTB property outside a node");
                let len = read_u32(remaining, 4) as usize;
                12usize
                    .checked_add(padded_size(len))
                    .expect("DTB property size overflow")
            }
            FDT_END_NODE => {
                depth = depth.checked_sub(1).expect("unmatched DTB end node");
                4
            }
            FDT_NOP => 4,
            FDT_END => {
                assert!(
                    saw_root && depth == 0 && remaining.len() == 4,
                    "invalid DTB end token"
                );
                4
            }
            _ => panic!("invalid DTB structure token"),
        };
        let token = remaining.get(..size).expect("truncated DTB token");
        visit(tag, token);
        remaining = &remaining[size..];
        if tag == FDT_END {
            return;
        }
    }
    panic!("missing DTB end token");
}
