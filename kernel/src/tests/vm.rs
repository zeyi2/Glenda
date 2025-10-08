use crate::printk;
use crate::printk::{ANSI_GREEN, ANSI_RESET, ANSI_YELLOW};

pub fn run(hartid: usize) {
    printk!("{}[TEST]{} VM test start (hart {})", ANSI_YELLOW, ANSI_RESET, hartid);
    if hartid != 0 {
        vm_func_test();
    }
    vm_mapping_test();
    printk!("{}[PASS]{} VM test", ANSI_GREEN, ANSI_RESET);
}

fn vm_func_test() {}

fn vm_mapping_test() {}
