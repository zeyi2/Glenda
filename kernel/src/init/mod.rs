mod harts;
mod pmem;

pub fn init(hartid: usize, dtb: *const u8) {
    init_harts(hartid, dtb);
    init_pmem(hartid, dtb);
}

fn init_pmem(hartid: usize, _dtb: *const u8) {
    pmem::pmem_init(hartid);
}

fn init_harts(hartid: usize, dtb: *const u8) {
    harts::bootstrap_secondary_harts(hartid, dtb);
}
