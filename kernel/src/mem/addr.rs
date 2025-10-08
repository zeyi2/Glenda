pub const PGSIZE: usize = 4096;

pub type PhysAddr = usize;
pub type VirtAddr = usize;

unsafe extern "C" {
    pub static mut __bss_end: u8;
}

#[inline(always)]
pub const fn align_up(value: usize) -> usize {
    debug_assert!(PGSIZE.is_power_of_two());
    (value + PGSIZE - 1) & !(PGSIZE - 1)
}

#[inline(always)]
pub const fn align_down(value: usize) -> usize {
    debug_assert!(PGSIZE.is_power_of_two());
    value & !(PGSIZE - 1)
}

#[inline(always)]
pub const fn ppn(addr: PhysAddr) -> [usize; 3] {
    [(addr >> 12) & 0x1FF, (addr >> 21) & 0x1FF, (addr >> 30) & 0x1FF]
}

#[inline(always)]
pub const fn vpn(addr: VirtAddr) -> [usize; 3] {
    [(addr >> 12) & 0x1FF, (addr >> 21) & 0x1FF, (addr >> 30) & 0x1FF]
}
