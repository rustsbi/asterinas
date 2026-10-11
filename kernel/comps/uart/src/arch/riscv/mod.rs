// SPDX-License-Identifier: MPL-2.0

use fdt::node::FdtNode;
use ostd::arch::boot::DEVICE_TREE;

mod ns16550a;

pub(super) fn init() {
    let device_tree = DEVICE_TREE.get().unwrap();

    // `fdt::Chosen::stdout` does not strip options such as `:115200n8`.
    let stdout = device_tree
        .find_node("/chosen")
        .and_then(|chosen| chosen.property("stdout-path"))
        .and_then(|prop| prop.as_str())
        .and_then(|path| device_tree.find_node(path.split(':').next()?));

    if let Some(node) = stdout.filter(|node| {
        is_available(*node)
            && node.compatible().is_some_and(|compatible| {
                compatible
                    .all()
                    .any(|name| matches!(name, "ns16550a" | "snps,dw-apb-uart"))
            })
    }) {
        ns16550a::init(node);
        return;
    }

    // DesignWare consoles rely on firmware's pinmux, clocks and baud rate, so
    // only claim one explicitly selected as the firmware console above.
    if let Some(node) = device_tree.all_nodes().find(|node| {
        is_available(*node)
            && node
                .compatible()
                .is_some_and(|compatible| compatible.all().any(|name| name == "ns16550a"))
    }) {
        ns16550a::init(node);
    }
}

fn is_available(node: FdtNode) -> bool {
    node.property("status")
        .is_none_or(|status| matches!(status.as_str(), Some("okay" | "ok")))
}
