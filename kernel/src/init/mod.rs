mod harts;
mod pmem;
mod vm;

pub fn init_pmem(_hartid: usize, _dtb: *const u8) {
    pmem::pmem_init();
}

pub fn init_harts(hartid: usize, dtb: *const u8) {
    harts::bootstrap_secondary_harts(hartid, dtb);
}

pub fn init_vm(hartid: usize, _dtb: *const u8) {
    vm::vm_init(hartid);
}
