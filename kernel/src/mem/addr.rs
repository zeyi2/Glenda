pub const PGSIZE: usize = 4096; // 4KB
pub const PGMASK: usize = PGSIZE - 1;
#[inline]
pub const fn align_up(addr: usize) -> usize {
    (addr + PGMASK) & !PGMASK
}
#[inline]
pub const fn align_down(addr: usize) -> usize {
    addr & !PGMASK
}

#[inline]
pub const fn ppn(pa: usize) -> usize {
    pa >> 12
}

#[inline]
pub const fn vpn(va: usize) -> usize {
    va >> 12
}

#[inline]
pub const fn page_offset(addr: usize) -> usize {
    addr & PGMASK
}

#[inline]
pub const fn vpn_indices(va: usize) -> [usize; 3] {
    [
        (va >> 12) & 0x1FF, // VPN[0]
        (va >> 21) & 0x1FF, // VPN[1]
        (va >> 30) & 0x1FF, // VPN[2]
    ]
}
