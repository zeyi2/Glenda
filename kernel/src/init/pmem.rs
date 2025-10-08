use crate::mem::pmem::initialize_regions;
use spin::Once;

static PMEM_ONCE: Once<()> = Once::new();
pub fn pmem_init(hartid: usize) {
    PMEM_ONCE.call_once(|| {
        initialize_regions(hartid);
    });
}
