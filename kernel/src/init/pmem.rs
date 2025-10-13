use crate::mem::pmem::initialize_regions;

pub fn pmem_init(hartid: usize) {
    initialize_regions(hartid);
}
