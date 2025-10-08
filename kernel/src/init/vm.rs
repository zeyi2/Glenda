use crate::mem::vm::{init_kernel_vm, switch_to_kernel_vm};
use crate::printk;
use spin::Once;

static VM_INIT_ONCE: Once<()> = Once::new();

pub fn vm_init(hartid: usize) {
    VM_INIT_ONCE.call_once(|| {
        init_kernel_vm();
        printk!("VM: Root page table built by hart {}", hartid);
    });

    switch_to_kernel_vm();
    printk!("VM: Hart {} switched to kernel page table", hartid);
}
