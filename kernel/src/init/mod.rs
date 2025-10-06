mod harts;
mod pmem;

pub fn init_pmem(hartid: usize, _dtb: *const u8) {
    pmem::pmem_init(hartid);
}

pub fn init_harts(hartid: usize, dtb: *const u8) {
    harts::bootstrap_secondary_harts(hartid, dtb);
}
