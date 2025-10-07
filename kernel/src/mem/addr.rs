/// Sv39 物理地址
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysAddr(pub usize);
/// Sv39 虚拟地址
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtAddr(pub usize);

use crate::mem::pmem::PGSIZE;
pub const PGMASK: usize = PGSIZE - 1;

impl PhysAddr {
    #[inline]
    pub const fn new(addr: usize) -> Self {
        PhysAddr(addr)
    }
    #[inline]
    pub const fn as_usize(&self) -> usize {
        self.0
    }
    /// 获取物理页号 (PPN)
    #[inline]
    pub const fn ppn(&self) -> usize {
        self.0 >> 12
    }
    /// 获取页偏移
    #[inline]
    pub const fn page_offset(&self) -> usize {
        self.0 & PGMASK
    }
    /// 由物理页号和偏移构造物理地址
    #[inline]
    pub const fn from_ppn_offset(ppn: usize, offset: usize) -> Self {
        PhysAddr((ppn << 12) | (offset & 0xfff))
    }
    // 对齐到页边界
    #[inline]
    pub const fn align_down(&self) -> Self {
        PhysAddr(self.0 & !PGMASK)
    }
    #[inline]
    pub const fn align_up(&self) -> Self {
        PhysAddr((self.0 + PGMASK) & !PGMASK)
    }
}

impl VirtAddr {
    #[inline]
    pub const fn new(addr: usize) -> Self {
        VirtAddr(addr)
    }
    #[inline]
    pub const fn as_usize(&self) -> usize {
        self.0
    }
    /// 获取虚拟页号 (VPN)
    #[inline]
    pub const fn vpn(&self) -> usize {
        self.0 >> 12
    }
    /// 获取页偏移
    #[inline]
    pub const fn page_offset(&self) -> usize {
        self.0 & 0xfff
    }
    /// 由虚拟页号和偏移构造虚拟地址
    #[inline]
    pub const fn from_vpn_offset(vpn: usize, offset: usize) -> Self {
        VirtAddr((vpn << 12) | (offset & 0xfff))
    }
    /// Sv39: 获取三级页表索引 (VPN[2], VPN[1], VPN[0])
    #[inline]
    pub const fn vpn_indices(&self) -> [usize; 3] {
        [
            (self.0 >> 30) & 0x1ff, // VPN[2]
            (self.0 >> 21) & 0x1ff, // VPN[1]
            (self.0 >> 12) & 0x1ff, // VPN[0]
        ]
    }
    // 对齐到页边界
    #[inline]
    pub const fn align_down(&self) -> Self {
        VirtAddr(self.0 & !PGMASK)
    }
    #[inline]
    pub const fn align_up(&self) -> Self {
        VirtAddr((self.0 + PGMASK) & !PGMASK)
    }
}
