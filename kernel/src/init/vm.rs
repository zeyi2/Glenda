use crate::mem::vm::{init_kernel_vm, vm_switch_to_kernel};

pub fn vm_init(hartid: usize) {
    init_kernel_vm(hartid);
    vm_switch_to_kernel(hartid);
}
