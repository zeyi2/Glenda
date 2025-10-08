#![allow(dead_code)]
use super::addr::{PGSIZE, align_down, align_up, vpn_indices};
use super::pmem::{kernel_region_info, pmem_alloc, pmem_free, user_region_info};
use super::pte::{PTE_R, PTE_V, PTE_W, PTE_X, PageTable, Pte};
use crate::printk;

// RISC-V QEMU virt 平台的内存映射布局常量
const UART_BASE: usize = 0x1000_0000;
const UART_SIZE: usize = 0x1000; // 4KB
const CLINT_BASE: usize = 0x0200_0000;
const CLINT_SIZE: usize = 0x10000; // 64KB
const PLIC_BASE: usize = 0x0C00_0000;
const PLIC_SIZE: usize = 0x400_0000; // 64MB
const KERNBASE: usize = 0x8020_0000;

unsafe extern "C" {
    static __text_start: u8;
    static __text_end: u8;
    static __rodata_start: u8;
    static __rodata_end: u8;
    static __data_start: u8;
}

use core::arch::asm;

pub const SATP_SV39: usize = 8 << 60; // SV39

// 存储内核页表的物理地址（而不是引用）
static mut KERNEL_PAGE_TABLE: usize = 0;
static KERNEL_PAGE_TABLE_LOCK: spin::Mutex<()> = spin::Mutex::new(());

/// 获取虚拟地址对应的 PTE 指针（多级页表遍历）
///
/// 遍历三级页表（Sv39），获取叶子节点的 PTE。
/// 如果 alloc 为 true，则在遍历过程中自动分配缺失的中间页表。
pub fn vm_getpte(root: &mut PageTable, va: usize, alloc: bool) -> Option<&mut Pte> {
    let idx = vpn_indices(va);
    let mut pt = root;

    // 遍历前两级页表（非叶子节点）
    for level in 0..2 {
        let pte = pt.get_pte(idx[level]);
        if !pte.is_valid() {
            if !alloc {
                return None;
            }
            // 分配下级页表
            let new_pt_pa = pmem_alloc(true);
            let new_pt = unsafe { &mut *(new_pt_pa as *mut PageTable) };
            *new_pt = PageTable::new();
            // 设置 PTE
            pt.map(idx[level], new_pt_pa as usize, PTE_V); // 只设置 V 位
        }
        // 获取下一级页表
        let pte = pt.get_pte(idx[level]);
        pt = unsafe { &mut *(pte.pa() as *mut PageTable) };
    }

    // 返回叶子节点的 PTE（第三级）
    Some(&mut pt.entries[idx[2]])
}

/// 映射虚拟地址区间到物理地址区间
///
/// 将 [va_start, va_start + size) 映射到 [pa_start, pa_start + size)。
/// 使用三级页表，按需分配中间页表。
pub fn vm_mappages(
    root: &mut PageTable,
    va_start: usize,
    pa_start: usize,
    size: usize,
    perm: usize,
) {
    let mut va = align_down(va_start);
    let mut pa = align_down(pa_start);
    let end = align_down(va + size - 1);

    while va < end {
        // 获取或创建对应的 PTE
        let pte = vm_getpte(root, va, true).expect("VM: PTE allocation failed");
        // 检查是否已映射
        if pte.is_valid() {
            panic!("vm_mappages: remap va {:#x}", va);
        }
        // 设置映射和权限
        *pte = Pte::new(pa, perm | PTE_V);
        va += PGSIZE;
        pa += PGSIZE;
    }
}

/// 取消映射虚拟地址区间
///
/// 取消 [va_start, va_start + size) 的映射，并释放对应的物理页。
pub fn vm_unmappages(root: &mut PageTable, va_start: usize, size: usize, free: bool) {
    let mut va = va_start;
    let end = va + size;

    while va < end {
        if let Some(pte) = vm_getpte(root, va, false) {
            if pte.is_valid() {
                // 释放物理页
                let pa = pte.pa();
                if free {
                    pmem_free(pa, false);
                }
                // 清除 PTE
                *pte = Pte(0);
            }
        }
        va += PGSIZE;
    }
}

/// 打印页表映射（递归遍历所有有效 PTE）
pub fn vm_print(pt: &PageTable, level: usize, va_base: usize) {
    for (i, pte) in pt.entries.iter().enumerate() {
        if pte.is_valid() {
            let va = va_base | (i << (12 + 9 * (2 - level)));
            if level == 2 || (pte.flags() & (PTE_R | PTE_W | PTE_X)) != 0 {
                // 叶子节点，输出映射
                printk!("VA {:#x} -> PA {:#x} [flags: {:#x}]", va, pte.pa(), pte.flags());
            } else {
                // 中间节点，递归
                let next_pt = unsafe { &*(pte.pa() as *const PageTable) };
                vm_print(next_pt, level + 1, va);
            }
        }
    }
}

// 初始化内核页表
pub fn init_kernel_page_table() {
    // 分配根页表
    let root_pa = pmem_alloc(true);
    let root = unsafe { &mut *(root_pa as *mut PageTable) };
    *root = PageTable::new();

    // 获取内核结束地址 (__bss_end)
    let kernel_end = crate::mem::pmem::kernel_region_info().begin;

    // 获取可分配区域的范围
    let kernel_region = kernel_region_info();
    let user_region = user_region_info();
    let alloc_start = kernel_region.begin;
    let alloc_end = user_region.end;

    // 1. 映射 UART (0x1000_0000, 设备寄存器, RW)
    map_range(root, UART_BASE, UART_BASE + UART_SIZE, PTE_R | PTE_W);
    printk!("VM: Mapped UART @ {:#x}", UART_BASE);

    // 2. 映射 CLINT (0x0200_0000, 设备寄存器, RW)
    map_range(root, CLINT_BASE, CLINT_BASE + CLINT_SIZE, PTE_R | PTE_W);
    printk!("VM: Mapped CLINT @ {:#x}", CLINT_BASE);

    // 3. 映射 PLIC (0x0C00_0000, 设备寄存器, RW)
    map_range(root, PLIC_BASE, PLIC_BASE + PLIC_SIZE, PTE_R | PTE_W);
    printk!("VM: Mapped PLIC @ {:#x}", PLIC_BASE);

    // 4. 映射不同的 section，防止触发页面保护异常
    // FIXME: This is basically C, RIIR!
    let text_start = unsafe { &__text_start as *const u8 as usize };
    let text_end = unsafe { &__text_end as *const u8 as usize };
    let rodata_start = unsafe { &__rodata_start as *const u8 as usize };
    let rodata_end = unsafe { &__rodata_end as *const u8 as usize };
    let data_start = unsafe { &__data_start as *const u8 as usize };

    map_range(root, text_start, text_end, PTE_R | PTE_X);
    printk!("VM: Mapped kernel text [{:#x}, {:#x})", text_start, align_up(text_end));

    map_range(root, rodata_start, rodata_end, PTE_R);
    printk!("VM: Mapped kernel rodata [{:#x}, {:#x})", rodata_start, align_up(rodata_end));

    map_range(root, data_start, kernel_end, PTE_R | PTE_W);
    printk!("VM: Mapped kernel data+bss [{:#x}, {:#x})", data_start, kernel_end);

    // 5. 映射可分配区域 (内核堆和用户内存池, RW)
    map_range(root, alloc_start, alloc_end, PTE_R | PTE_W);
    printk!("VM: Mapped allocable region [{:#x}, {:#x})", alloc_start, alloc_end);

    // 保存根页表的物理地址到全局变量
    let _guard = KERNEL_PAGE_TABLE_LOCK.lock();
    unsafe {
        KERNEL_PAGE_TABLE = root_pa as usize;
    }
    printk!("VM: Kernel page table initialized at PA {:#x}", root_pa as usize);
}

#[inline]
fn map_range(root: &mut PageTable, start: usize, end: usize, perm: usize) {
    if start >= end {
        return;
    }
    let va_start = align_down(start);
    let va_end = align_up(end);
    let size = va_end - va_start;
    vm_mappages(root, va_start, va_start, size, perm);
}

// 切换到内核页表
pub fn switch_to_kernel_page_table(hartid: usize) {
    let _guard = KERNEL_PAGE_TABLE_LOCK.lock();
    let root_pa = unsafe { KERNEL_PAGE_TABLE };
    let satp = SATP_SV39 | root_pa >> 12;
    unsafe {
        asm!("csrw satp, {}", in(reg) satp);
        asm!("sfence.vma zero, zero");
    }
    printk!("VM: Switched to kernel page table (SATP={:#x}) on hart {}", satp, hartid);
}
