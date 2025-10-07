#![allow(dead_code)]
//! RISC-V Sv39 页表项与页表操作
use super::addr::{PhysAddr, VirtAddr};

/// Sv39 页表项 (PTE)
#[derive(Clone, Copy)]
pub struct Pte(pub usize);

// PTE 标志位
pub const PTE_V: usize = 1 << 0; // 有效位
pub const PTE_R: usize = 1 << 1; // 读
pub const PTE_W: usize = 1 << 2; // 写
pub const PTE_X: usize = 1 << 3; // 执行
pub const PTE_U: usize = 1 << 4; // 用户
pub const PTE_G: usize = 1 << 5; // 全局
pub const PTE_A: usize = 1 << 6; // 已访问
pub const PTE_D: usize = 1 << 7; // 脏页

pub const PAGE_SIZE: usize = 4096;
pub const PTE_COUNT: usize = 512; // 每级页表项数

impl Pte {
    pub fn new(pa: PhysAddr, flags: usize) -> Self {
        Pte((pa.ppn() << 10) | (flags & 0x3FF))
    }
    pub fn is_valid(&self) -> bool {
        self.0 & PTE_V != 0
    }
    pub fn flags(&self) -> usize {
        self.0 & 0x3FF
    }
    pub fn pa(&self) -> PhysAddr {
        PhysAddr::from_ppn_offset((self.0 >> 10) & ((1 << 44) - 1), 0)
    }
}

/// Sv39 页表结构
pub struct PageTable {
    pub entries: [Pte; PTE_COUNT],
}

impl PageTable {
    pub const fn new() -> Self {
        Self { entries: [Pte(0); PTE_COUNT] }
    }

    pub fn map(&mut self, vpn: usize, pa: PhysAddr, flags: usize) {
        self.entries[vpn] = Pte::new(pa, flags | PTE_V);
    }

    pub fn unmap(&mut self, vpn: usize) {
        self.entries[vpn] = Pte(0);
    }

    pub fn get_pte(&self, vpn: usize) -> &Pte {
        &self.entries[vpn]
    }
}
