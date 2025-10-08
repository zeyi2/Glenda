mod pmem;
mod printk;
mod spinlock;
mod vm;

pub fn run_tests(hartid: usize) {
    run_spinlock_tests(hartid);
    run_printk_tests(hartid);
    run_pmem_tests(hartid);
}

fn run_spinlock_tests(hartid: usize) {
    spinlock::run(hartid);
}
fn run_printk_tests(hartid: usize) {
    if hartid != 0 {
        return;
    }
    printk::run();
}
fn run_pmem_tests(hartid: usize) {
    pmem::run(hartid);
}

fn run_vm_tests(hartid: usize) {
    vm::run(hartid);
}
