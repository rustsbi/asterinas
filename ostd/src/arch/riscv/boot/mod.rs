// SPDX-License-Identifier: MPL-2.0

//! The RISC-V boot module defines the entrypoints of Asterinas.

mod device_tree;
pub(crate) mod smp;

use core::arch::global_asm;

use fdt::Fdt;
use spin::Once;

use crate::{
    boot::{
        BootloaderAcpiArg, BootloaderFramebufferArg,
        memory_region::{MemoryRegion, MemoryRegionArray, MemoryRegionType},
    },
    early_println,
    mm::paddr_to_vaddr,
};

#[cfg(target_arch = "riscv64")]
global_asm!(include_str!("bsp_boot.S"));
#[cfg(target_arch = "riscv32")]
global_asm!(include_str!("bsp_boot32.S"), early_boot_trap = sym early_boot_trap);

#[cfg(target_arch = "riscv32")]
extern "C" fn early_boot_trap(cause: usize, epc: usize, tval: usize) -> ! {
    early_println!(
        "Early boot trap: scause={:#x}, sepc={:#x}, stval={:#x}",
        cause,
        epc,
        tval
    );
    loop {
        // SAFETY: Interrupts remain disabled on this unrecoverable boot fault.
        unsafe { core::arch::asm!("wfi") };
    }
}

/// The Flattened Device Tree of the platform.
pub static DEVICE_TREE: Once<Fdt> = Once::new();

fn parse_bootloader_name() -> &'static str {
    "Unknown"
}

fn parse_kernel_commandline() -> &'static str {
    DEVICE_TREE.get().unwrap().chosen().bootargs().unwrap_or("")
}

fn parse_initramfs() -> Option<&'static [u8]> {
    let (start, end) = parse_initramfs_range()?;

    let base_va = paddr_to_vaddr(start);
    let length = end - start;
    // SAFETY:
    // 1. The initramfs is safe to read because of the contract with the loader.
    // 2. We reserve the initramfs region in `parse_memory_regions`, so it will live as an immutable
    //    reference for `'static`.
    Some(unsafe { core::slice::from_raw_parts(base_va as *const u8, length) })
}

fn parse_acpi_arg() -> BootloaderAcpiArg {
    // TDDO: Add ACPI support for RISC-V, maybe.
    BootloaderAcpiArg::NotProvided
}

fn parse_framebuffer_info() -> Option<BootloaderFramebufferArg> {
    // TODO: Parse framebuffer info from device tree.
    None
}

fn parse_memory_regions(device_tree_paddr: usize) -> MemoryRegionArray {
    let mut regions = MemoryRegionArray::new();
    let fdt = DEVICE_TREE.get().unwrap();

    for region in DEVICE_TREE.get().unwrap().memory().regions() {
        if region.size.unwrap_or(0) > 0 {
            regions
                .push(MemoryRegion::new(
                    region.starting_address as usize,
                    region.size.unwrap(),
                    MemoryRegionType::Usable,
                ))
                .unwrap();
        }
    }

    // The firmware may reserve its runtime memory in the FDT reservation map
    // instead of a /reserved-memory node. Both forms must reach the allocator.
    for reservation in fdt.memory_reservations() {
        regions
            .push(MemoryRegion::new(
                reservation.address() as usize,
                reservation.size(),
                MemoryRegionType::Reserved,
            ))
            .unwrap();
    }

    // DEVICE_TREE and boot arguments may borrow the loader's blob for 'static.
    // Do not let the frame allocator reuse it even if the loader did not
    // explicitly reserve the blob itself.
    regions
        .push(MemoryRegion::new(
            device_tree_paddr,
            fdt.total_size(),
            MemoryRegionType::Reserved,
        ))
        .unwrap();

    if let Some(node) = DEVICE_TREE.get().unwrap().find_node("/reserved-memory") {
        for child in node.children() {
            if let Some(reg_iter) = child.reg() {
                for region in reg_iter {
                    regions
                        .push(MemoryRegion::new(
                            region.starting_address as usize,
                            region.size.unwrap(),
                            MemoryRegionType::Reserved,
                        ))
                        .unwrap();
                }
            }
        }
    }

    // Add the kernel region.
    regions.push(MemoryRegion::kernel()).unwrap();

    // Add the initramfs region.
    if let Some((start, end)) = parse_initramfs_range() {
        regions
            .push(MemoryRegion::new(
                start,
                end - start,
                MemoryRegionType::Module,
            ))
            .unwrap();
    }

    regions.into_non_overlapping()
}

fn parse_initramfs_range() -> Option<(usize, usize)> {
    let chosen = DEVICE_TREE.get().unwrap().find_node("/chosen")?;
    let initrd_start = chosen.property("linux,initrd-start")?.as_usize()?;
    let initrd_end = chosen.property("linux,initrd-end")?.as_usize()?;
    (initrd_start < initrd_end).then_some((initrd_start, initrd_end))
}

static mut BOOTSTRAP_HART_ID: u32 = u32::MAX;

/// The entry point of the Rust code portion of Asterinas.
///
/// `BOOTSTRAP_HART_ID` is initialized to be `hart_id` and accessible after calling this.
///
/// # Safety
///
/// - This function must be called only once at a proper timing in the BSP's boot assembly code.
/// - The caller must follow C calling conventions and put the right arguments in registers.
// SAFETY: The name does not collide with other symbols.
#[unsafe(no_mangle)]
unsafe extern "C" fn riscv_boot(hart_id: usize, device_tree_paddr: usize) -> ! {
    // We will only write it once. Other processors will only read it.
    // SAFETY: We don't create Rust references, so there are no aliasing problems. Other processors
    // have not been booted yet, so there are no data races.
    unsafe { BOOTSTRAP_HART_ID = hart_id as u32 };

    let device_tree_ptr = paddr_to_vaddr(device_tree_paddr) as *const u8;
    // SAFETY: The caller provides a valid immutable DTB. Only the BSP calls
    // this entrypoint, once, before starting other harts or publishing the tree.
    let fdt = unsafe { device_tree::from_ptr(device_tree_ptr) };
    DEVICE_TREE.call_once(|| fdt);

    use crate::boot::{EARLY_INFO, EarlyBootInfo, start_kernel};

    EARLY_INFO.call_once(|| EarlyBootInfo {
        bootloader_name: parse_bootloader_name(),
        kernel_cmdline: parse_kernel_commandline(),
        initramfs: parse_initramfs(),
        acpi_arg: parse_acpi_arg(),
        framebuffer_arg: parse_framebuffer_info(),
        memory_regions: parse_memory_regions(device_tree_paddr),
    });

    // SAFETY: The safety is guaranteed by the safety preconditions and the fact that we call it
    // once after setting up necessary resources.
    unsafe { start_kernel() };
}
