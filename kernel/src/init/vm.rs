use crate::mem::vm::{init_kernel_vm, vm_switch_to_kernel};
use crate::printk;
use spin::Once;

static VM_INIT_ONCE: Once<()> = Once::new();

pub fn vm_init(hartid: usize) {
    VM_INIT_ONCE.call_once(|| {
        init_kernel_vm();
        printk!("VM: Root page table built by hart {}", hartid);
    });

    vm_switch_to_kernel();
    printk!("VM: Hart {} switched to kernel page table", hartid);
}
