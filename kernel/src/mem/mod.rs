pub const PGSIZE: usize = 4096;
pub const PGNUM: usize = 512; // 2^9
pub const PGMASK: usize = PGSIZE - 1;
pub const KERN_PAGES: usize = 1024;
pub const VA_MAX: usize = 1 << 38;

pub mod addr;
pub mod pmem;
pub mod pte;
pub mod vm;
