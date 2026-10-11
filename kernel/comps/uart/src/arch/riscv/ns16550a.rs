// SPDX-License-Identifier: MPL-2.0

use alloc::string::ToString;

use fdt::node::FdtNode;
use ostd::{
    arch::{
        boot::DEVICE_TREE,
        irq::{IRQ_CHIP, InterruptSourceInFdt, MappedIrqLine},
    },
    console::uart_ns16650a::{Ns16550aAccess, Ns16550aRegister, Ns16550aUart},
    io::IoMem,
    irq::IrqLine,
    mm::VmIoOnce,
    sync::SpinLock,
};
use spin::Once;

use crate::console::{Uart, UartConsole};

/// IRQ line for UART serial.
static IRQ_LINE: Once<MappedIrqLine> = Once::new();

pub(super) fn init(fdt_node: FdtNode) {
    let Some(reg) = fdt_node.reg().and_then(|mut regs| regs.next()) else {
        ostd::info!("Failed to read 'reg' property from NS16550A node");
        return;
    };
    let Some(reg_size) = reg.size else {
        ostd::info!("Incomplete 'reg' property found in NS16550A node");
        return;
    };

    let reg_addr = reg.starting_address as usize;
    let Some(reg_end) = reg_addr.checked_add(reg_size) else {
        ostd::warn!("Invalid register range for NS16550A");
        return;
    };
    let Some(layout) = RegisterLayout::from_fdt(fdt_node, reg_addr, reg_size) else {
        ostd::warn!("Unsupported register layout for NS16550A");
        return;
    };
    let Some(interrupt_source) = interrupt_source(fdt_node) else {
        ostd::warn!("Unsupported interrupt source for NS16550A");
        return;
    };
    let Ok(io_mem) = IoMem::acquire(reg_addr..reg_end) else {
        ostd::info!("I/O memory is not available for NS16550A");
        return;
    };

    let mut access = SerialAccess { io_mem, layout };
    if layout.is_dw_apb && access.read(Ns16550aRegister::LineCtrl) & DLAB != 0 {
        // LCR writes can be ignored while a DesignWare UART is busy. Refuse
        // the handoff rather than risk accessing DLL/DLH as data/IER.
        ostd::warn!("DesignWare console requires firmware to clear DLAB");
        return;
    }

    let Ok(mut irq_line) = IrqLine::alloc() else {
        ostd::info!("IRQ line is not available for NS16550A");
        return;
    };

    if layout.is_dw_apb {
        // The UART clock is not the PC-compatible 1.8432 MHz clock assumed by
        // `Ns16550aUart::init`. Keep firmware's divisor and framing, but enable
        // FIFOs with the one-byte RX trigger before enabling receive interrupts.
        // FCR is write-only; the read alias is IIR, whose high bits report FIFO state.
        let fifo_before = access.read_register(Ns16550aRegister::FifoCtrl as usize) & 0xc0;
        access.write(Ns16550aRegister::FifoCtrl, FIFO_ENABLE);
        let fifo_after = access.read_register(Ns16550aRegister::FifoCtrl as usize) & 0xc0;
        ostd::info!("DesignWare UART FIFO status: {fifo_before:#04x} -> {fifo_after:#04x}");
        access.write(Ns16550aRegister::IntEnOrDivisorHi, RX_DATA_AVAILABLE);
    }

    let irq_access = access.clone();
    let mut uart = Ns16550aUart::new(access);
    if !layout.is_dw_apb {
        uart.init();
    }

    let uart_console = UartConsole::new(SpinLock::new(uart));

    let cloned_uart_console = uart_console.clone();
    irq_line.on_active(move |_| {
        {
            // The cloned MMIO view shares the UART lock with send/receive operations.
            let _uart = cloned_uart_console.uart().lock();
            irq_access.acknowledge_interrupt();
        }
        cloned_uart_console.trigger_input_callbacks();
    });

    // Install the handler before the PLIC enables the source.
    let Ok(irq_line) = IRQ_CHIP
        .get()
        .unwrap()
        .map_fdt_pin_to(interrupt_source, irq_line)
    else {
        ostd::info!("IRQ source is not available for NS16550A");
        return;
    };
    IRQ_LINE.call_once(move || irq_line);

    aster_console::register_device(
        aster_console::UART_CONSOLE_NAME.to_string(),
        uart_console.clone(),
    );

    uart_console.uart().flush();

    ostd::info!("Registered NS16550A as a console");
}

const DLAB: u8 = 1 << 7;
const RX_DATA_AVAILABLE: u8 = 1;
const FIFO_ENABLE: u8 = 1;
const DW_UART_USR: usize = 0x1f;
const DW_UART_IIR_BUSY: u8 = 0x07;
const DW_UART_IIR_RX_TIMEOUT: u8 = 0x0c;

/// The validated register stride and transaction width of a serial port.
#[derive(Clone, Copy)]
struct RegisterLayout {
    stride: usize,
    width: usize,
    is_dw_apb: bool,
}

impl RegisterLayout {
    fn from_fdt(node: FdtNode, base: usize, size: usize) -> Option<Self> {
        let shift = node
            .property("reg-shift")
            .map_or(Some(0), |prop| prop.as_usize())?;
        let width = node
            .property("reg-io-width")
            .map_or(Some(1), |prop| prop.as_usize())?;
        let stride = 1usize.checked_shl(u32::try_from(shift).ok()?)?;
        let is_dw_apb = node
            .compatible()?
            .all()
            .any(|name| name == "snps,dw-apb-uart");
        let last_register = if is_dw_apb {
            DW_UART_USR
        } else {
            Ns16550aRegister::ModemStat as usize
        };
        let required_size = last_register.checked_mul(stride)?.checked_add(width)?;

        if !matches!(width, 1 | 4)
            || stride < width
            || base % width != 0
            || required_size > size
            || node.property("big-endian").is_some()
            || node.property("reg-offset").is_some()
        {
            return None;
        }

        Some(Self {
            stride,
            width,
            is_dw_apb,
        })
    }
}

/// Access to serial registers via `IoMem`, serialized by the console's lock.
#[derive(Clone)]
struct SerialAccess {
    io_mem: IoMem,
    layout: RegisterLayout,
}

impl SerialAccess {
    fn read_register(&self, index: usize) -> u8 {
        let offset = index * self.layout.stride;
        if self.layout.width == 4 {
            u32::from_le(self.io_mem.read_once::<u32>(offset).unwrap()) as u8
        } else {
            self.io_mem.read_once(offset).unwrap()
        }
    }

    fn acknowledge_interrupt(&self) {
        if !self.layout.is_dw_apb {
            return;
        }

        // Reference: `dw8250_handle_irq` and `DW_UART_USR` in Linux:
        // https://github.com/YuzukiHD/linux-mainline/tree/735f2ca14b7b916b17fa111a941a9a79d0c12e61/drivers/tty/serial/8250
        let interrupt_id = self.read_register(Ns16550aRegister::FifoCtrl as usize) & 0x3f;
        if interrupt_id == DW_UART_IIR_BUSY {
            // IIR reports a busy-detect interrupt even when there is no input.
            // Reading USR acknowledges it before the receive loop returns.
            let _ = self.read_register(DW_UART_USR);
        }

        let line_status = self.read_register(Ns16550aRegister::LineStat as usize);
        const DATA_READY_OR_BREAK: u8 = (1 << 0) | (1 << 4);
        if interrupt_id == DW_UART_IIR_RX_TIMEOUT && line_status & DATA_READY_OR_BREAK == 0 {
            // Some DesignWare UARTs assert RX timeout with an empty FIFO.
            // A dummy RBR read clears it; do not deliver the dummy byte.
            let _ = self.read_register(Ns16550aRegister::DataOrDivisorLo as usize);
        }
    }
}

impl Ns16550aAccess for SerialAccess {
    fn read(&self, reg: Ns16550aRegister) -> u8 {
        // In particular, TX polling of LSR must never consume receive data.
        self.read_register(reg as usize)
    }

    fn write(&mut self, reg: Ns16550aRegister, val: u8) {
        let offset = reg as usize * self.layout.stride;
        if self.layout.width == 4 {
            self.io_mem
                .write_once(offset, &u32::from(val).to_le())
                .unwrap();
        } else {
            self.io_mem.write_once(offset, &val).unwrap();
        }
    }
}

fn interrupt_source(node: FdtNode) -> Option<InterruptSourceInFdt> {
    let device_tree = DEVICE_TREE.get()?;
    let (phandle, specifier) = if let Some(prop) = node.property("interrupts-extended") {
        (read_cell(prop.value)?, prop.value.get(4..)?)
    } else {
        (
            read_cell(node.property("interrupt-parent")?.value)?,
            node.property("interrupts")?.value,
        )
    };
    let controller = device_tree.find_phandle(phandle)?;
    let cells = controller.interrupt_cells()?;
    if !matches!(cells, 1 | 2) || specifier.len() < cells * 4 {
        return None;
    }
    // A two-cell PLIC binding includes a trigger type. UART uses level-high;
    // do not silently interpret a flags cell as part of the source number.
    const IRQ_TYPE_LEVEL_HIGH: u32 = 4;
    if cells == 2 && read_cell(specifier.get(4..)?)? != IRQ_TYPE_LEVEL_HIGH {
        return None;
    }
    let interrupt = read_cell(specifier)?;
    let num_sources = controller.property("riscv,ndev")?.as_usize()?;
    if interrupt == 0 || interrupt as usize > num_sources {
        return None;
    }

    Some(InterruptSourceInFdt {
        interrupt_parent: phandle,
        interrupt,
    })
}

fn read_cell(value: &[u8]) -> Option<u32> {
    Some(u32::from_be_bytes(value.get(..4)?.try_into().ok()?))
}
