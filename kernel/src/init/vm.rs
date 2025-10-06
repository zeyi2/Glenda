use crate::mem::vm::{init_kernel_page_table, switch_to_kernel_page_table};
use spin::Once;

static VM_ONCE: Once<()> = Once::new();

pub fn vm_init() {
    VM_ONCE.call_once(|| {
        init_kernel_page_table();
    });
    while VM_ONCE.is_completed() == false {
        core::hint::spin_loop();
    }
    switch_to_kernel_page_table();
}
