use crate::mem::vm::{init_kernel_vm, switch_to_kernel_vm};
use crate::printk;
use spin::Once;

static VM_INIT_ONCE: Once<()> = Once::new();

pub fn vm_init(hartid: usize) {
    VM_INIT_ONCE.call_once(|| {
        init_kernel_vm();
        crate::printk!("VM: Root page table built by hart{}\n", hartid);
    });

    switch_to_kernel_vm();
}
