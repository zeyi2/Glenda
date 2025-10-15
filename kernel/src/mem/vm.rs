#![allow(dead_code)]

use core::arch::asm;
use core::cmp::max;
use core::panic;

use super::addr::{PhysAddr, VirtAddr, align_down, align_up, vpn};
use super::pmem::{kernel_region_info, pmem_alloc, pmem_free, user_region_info};
use super::pte::{PTE_A, PTE_D, PTE_R, PTE_V, PTE_W, PTE_X, Pte};
use super::pte::{pa_to_pte, pte_get_flags, pte_is_leaf, pte_is_table, pte_is_valid, pte_to_pa};
use super::{PGNUM, PGSIZE, VA_MAX};
use crate::dtb;
use crate::printk;
use spin::Once;

const SATP_SV39: usize = 8 << 60;

// align 4096, 防止 sfence.vma 直接 TRAP
#[repr(C, align(4096))]
#[derive(Clone, Copy)]
pub struct PageTable {
    entries: [Pte; PGNUM],
}

// see linker.ld
unsafe extern "C" {
    static __text_start: u8;
    static __text_end: u8;
    static __rodata_start: u8;
    static __rodata_end: u8;
    static __data_start: u8;
    static __data_end: u8;
    static __bss_start: u8;
    static __bss_end: u8;
}

unsafe impl Sync for PageTable {}

static KERNEL_PAGE_TABLE: PageTable = PageTable { entries: [0; PGNUM] };

impl PageTable {
    // walk: 只支持 4KB 页；中间层遇到 leaf(=大页) 视为错误返回 None
    fn walk(&self, va: VirtAddr, alloc: bool) -> Option<*mut Pte> {
        if va >= VA_MAX {
            return None;
        }
        let mut table = self as *const PageTable as *mut PageTable;
        // 访问顺序：L2 -> L1，最后返回 L0 的 PTE 指针
        for level in (1..3).rev() {
            let idx = vpn(va)[level];
            let pte_ref = unsafe { &mut (*table).entries[idx] };
            if pte_is_valid(*pte_ref) {
                if pte_is_leaf(*pte_ref) {
                    // 不支持大页
                    return None;
                }
                // 进入下一层表
                table = pte_to_pa(*pte_ref) as *mut PageTable;
            } else {
                if !alloc {
                    return None;
                }
                let new_table = pmem_alloc(true) as *mut PageTable;
                if new_table.is_null() {
                    return None;
                }
                unsafe {
                    core::ptr::write_bytes(new_table as *mut u8, 0, PGSIZE);
                    *pte_ref = pa_to_pte(new_table as usize, PTE_V); // 仅 V 置位表示中间层
                }
                table = new_table;
            }
        }
        Some(unsafe { &mut (*table).entries[vpn(va)[0]] as *mut Pte })
    }

    fn map(&self, va: VirtAddr, pa: PhysAddr, len: usize, flags: usize) -> bool {
        if len == 0 {
            return false;
        }
        let start = align_down(va);
        let end = align_up(va + len);
        let mut a = start;
        let mut pa_cur = align_down(pa);
        let last = end - PGSIZE;
        while a <= last {
            let pte = match self.walk(a, true) {
                Some(p) => p,
                None => return false,
            };
            let cur = unsafe { *pte };
            if pte_is_valid(cur) {
                // 已存在映射：允许对同一物理页更新权限；若物理页不同则视为冲突
                if !pte_is_leaf(cur) || pte_to_pa(cur) != pa_cur {
                    return false; // 冲突或结构错误
                }
                unsafe {
                    *pte = pa_to_pte(pa_cur, flags | PTE_V);
                }
            } else {
                unsafe {
                    *pte = pa_to_pte(pa_cur, flags | PTE_V);
                }
            }
            if a == last {
                break;
            }
            a += PGSIZE;
            pa_cur += PGSIZE;
        }
        true
    }

    fn unmap(&self, va: VirtAddr, len: usize, free: bool) -> bool {
        if len == 0 {
            return false;
        }
        let start = align_down(va);
        let end = align_up(va + len);
        let mut a = start;
        let last = end - PGSIZE;
        while a <= last {
            let pte = match self.walk(a, false) {
                Some(p) => p,
                None => return false,
            };
            let old = unsafe { *pte };
            if !pte_is_valid(old) || !pte_is_leaf(old) {
                return false;
            }
            let pa = pte_to_pa(old);
            if free {
                match get_region(pa) {
                    Some(for_kernel) => pmem_free(pa, for_kernel),
                    None => panic!("vm_unmappages: PA {:#x} out of bounds", pa),
                };
            }
            unsafe { *pte = 0 }; // 清除映射
            if a == last {
                break;
            }
            a += PGSIZE;
        }
        true
    }
}

pub fn vm_getpte(table: &PageTable, va: VirtAddr) -> *mut Pte {
    match table.walk(va, false) {
        Some(p) => p,
        None => panic!("vm_getpte: failed for VA {:#x}", va),
    }
}

fn get_region(pa: PhysAddr) -> Option<bool> {
    let kern_region = kernel_region_info();
    let user_region = user_region_info();
    if pa >= kern_region.begin && pa < kern_region.end {
        return Some(true);
    } else if pa >= user_region.begin && pa < user_region.end {
        return Some(false);
    } else {
        return None;
    }
}

pub fn vm_mappages(table: &PageTable, va: VirtAddr, size: usize, pa: PhysAddr, perm: usize) {
    if !table.map(va, pa, size, perm) {
        panic!("vm_mappages: failed map VA {:#x} -> PA {:#x}", va, pa);
    }
}

pub fn vm_unmappages(table: &PageTable, va: VirtAddr, size: usize, free: bool) {
    if !table.unmap(va, size, free) {
        panic!("vm_unmappages: failed unmap VA {:#x}", va);
    }
}

#[cfg(feature = "tests")]
pub fn vm_print(table: &PageTable) {
    // 打印三级页表，仅支持 4KB 页；显示每级基准 VA
    let pgtbl_2 = table; // level-2 (root)
    printk!("level-2 pgtbl: pa = {:p}", pgtbl_2);
    for i in 0..PGNUM {
        let pte2 = pgtbl_2.entries[i];
        if !pte_is_valid(pte2) {
            continue;
        }
        if !pte_is_table(pte2) {
            panic!("vm_print: pte check fail (1) L2 idx {} PTE {:#x}", i, pte2);
        }
        let pgtbl_1 = pte_to_pa(pte2) as *const PageTable;
        let base_va_l2 = (i << 30) as *const u8;
        printk!(".. level-1 pgtbl {:3} base_va = {:p} pa = {:p}", i, base_va_l2, pgtbl_1);
        for j in 0..PGNUM {
            let pte1 = unsafe { (*pgtbl_1).entries[j] };
            if !pte_is_valid(pte1) {
                continue;
            }
            if !pte_is_table(pte1) {
                panic!("vm_print: pte check fail (2) L1 idx {} PTE {:#x}", j, pte1);
            }
            let pgtbl_0 = pte_to_pa(pte1) as *const PageTable;
            let base_va_l1 = ((i << 30) | (j << 21)) as *const u8;
            printk!(".. .. level-0 pgtbl {:3} base_va = {:p} pa = {:p}", j, base_va_l1, pgtbl_0);
            for k in 0..PGNUM {
                let pte0 = unsafe { (*pgtbl_0).entries[k] };
                if !pte_is_valid(pte0) {
                    continue;
                }
                if !pte_is_leaf(pte0) {
                    panic!("vm_print: pte check fail (3) L0 idx {} PTE {:#x}", k, pte0);
                }
                let pa = pte_to_pa(pte0);
                let va = ((i << 30) | (j << 21) | (k << 12)) as *const u8;
                let flags = pte_get_flags(pte0);
                printk!(
                    ".. .. .. page {:3} VA {:p} -> PA {:p} flags = {:#x}",
                    k,
                    va,
                    pa as *const u8,
                    flags
                );
            }
        }
    }
}

#[inline(always)]
fn make_satp(ppn: usize) -> usize {
    SATP_SV39 | ppn
}

static INIT_KERNEL_VM: Once<()> = Once::new();

pub fn init_kernel_vm(hartid: usize) {
    INIT_KERNEL_VM.call_once(|| {
        __init_kernel_vm(hartid);
    });
}

fn __init_kernel_vm(hartid: usize) {
    // 权限映射, PTE_A/D 理论上硬件会帮忙做，但不确定 QEMU Virt 的具体行为，所以还是加上
    let text_start_addr = unsafe { &__text_start as *const u8 as usize };
    let text_end_addr = unsafe { &__text_end as *const u8 as usize };
    printk!("VM: Map .text [{:p}, {:p})", text_start_addr as *const u8, text_end_addr as *const u8);
    vm_mappages(
        &KERNEL_PAGE_TABLE,
        text_start_addr,
        text_end_addr - text_start_addr,
        text_start_addr,
        PTE_R | PTE_X | PTE_A,
    );

    let rodata_start_addr = unsafe { &__rodata_start as *const u8 as usize };
    let rodata_end_addr = unsafe { &__rodata_end as *const u8 as usize };
    printk!(
        "VM: Map .rodata [{:p}, {:p})",
        rodata_start_addr as *const u8,
        rodata_end_addr as *const u8
    );
    vm_mappages(
        &KERNEL_PAGE_TABLE,
        rodata_start_addr,
        rodata_end_addr - rodata_start_addr,
        rodata_start_addr,
        PTE_R | PTE_A,
    );

    let data_start_addr = unsafe { &__data_start as *const u8 as usize };
    let data_end_addr = unsafe { &__data_end as *const u8 as usize };
    printk!("VM: Map .data [{:p}, {:p})", data_start_addr as *const u8, data_end_addr as *const u8);
    vm_mappages(
        &KERNEL_PAGE_TABLE,
        data_start_addr,
        data_end_addr - data_start_addr,
        data_start_addr,
        PTE_R | PTE_W | PTE_A | PTE_D,
    );

    let bss_start_addr = unsafe { &__bss_start as *const u8 as usize };
    let bss_end_addr = unsafe { &__bss_end as *const u8 as usize };
    printk!("VM: Map .bss [{:p}, {:p})", bss_start_addr as *const u8, bss_end_addr as *const u8);
    vm_mappages(
        &KERNEL_PAGE_TABLE,
        bss_start_addr,
        bss_end_addr - bss_start_addr,
        bss_start_addr,
        PTE_R | PTE_W | PTE_A | PTE_D,
    );

    // MMIO 映射
    let uart_base = dtb::uart_config().unwrap_or(driver_uart::DEFAULT_QEMU_VIRT).base();
    let uart_size = PGSIZE;
    printk!("VM: Map UART @ {:p}", uart_base as *const u8);
    vm_mappages(&KERNEL_PAGE_TABLE, uart_base, uart_size, uart_base, PTE_R | PTE_W | PTE_A | PTE_D);

    // 内核的物理页分配池
    let kernel_info = kernel_region_info();
    let map_start = align_down(kernel_info.begin);
    let map_end = align_up(kernel_info.end);
    if map_start < map_end {
        printk!("VM: Map kernel pool [{:p}, {:p})", map_start as *const u8, map_end as *const u8);
        vm_mappages(
            &KERNEL_PAGE_TABLE,
            map_start,
            map_end - map_start,
            map_start,
            PTE_R | PTE_W | PTE_A | PTE_D,
        );
    }
    // FIXME: 不应该这么做，目前仅为过测试
    let user = user_region_info();
    let user_start = align_down(user.begin);
    let user_end = align_up(user.end);
    if user_start < user_end {
        printk!("VM: Map user pool [{:p}, {:p})", user_start as *const u8, user_end as *const u8);
        vm_mappages(
            &KERNEL_PAGE_TABLE,
            user_start,
            user_end - user_start,
            user_start,
            PTE_R | PTE_W | PTE_A | PTE_D
        );
    }
    printk!("VM: Root page table built by hart {}", hartid);
}

pub fn vm_switch_to_kernel(hartid: usize) {
    let root_pa = (&KERNEL_PAGE_TABLE as *const PageTable) as usize;
    unsafe {
        asm!("csrw satp, {}", in(reg) make_satp(root_pa >> 12));
        asm!("sfence.vma zero, zero");
    }
    printk!("VM: Hart {} switched to kernel page table", hartid);
}

pub fn vm_switch_off() {
    unsafe {
        asm!("csrw satp, zero");
        asm!("sfence.vma zero, zero");
    }
}
