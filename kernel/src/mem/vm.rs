#![allow(dead_code)]

use core::arch::asm;
use core::panic;

use super::addr::{PhysAddr, VirtAddr, align_down, align_up, vpn};
use super::pmem::{pmem_alloc, pmem_free};
use super::pte::{PTE_V, Pte};
use super::pte::{pa_to_pte, pte_check, pte_is_valid, pte_to_pa};
use super::{PGNUM, PGSIZE, VA_MAX};
use crate::printk;

const SATP_SV39: usize = 8 << 60;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PageTable {
    entries: [Pte; PGNUM],
}

unsafe impl Sync for PageTable {}

static KERNEL_PAGE_TABLE: PageTable = PageTable { entries: [0; PGNUM] };

impl PageTable {
    fn walk(&self, va: VirtAddr, alloc: bool) -> Option<*mut Pte> {
        if va >= VA_MAX {
            return None;
        }
        let mut table = self as *const PageTable as *mut PageTable;
        for level in (1..3).rev() {
            let pte = unsafe { &mut (*table).entries[vpn(va)[level]] };
            if pte_is_valid(*pte) {
                table = pte_to_pa(*pte) as *mut PageTable;
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
                    *pte = (new_table as usize >> 12) << 10 | PTE_V;
                    table = new_table;
                }
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
        let mut pa = align_down(pa);
        let last = end - PGSIZE;
        while a <= last {
            let pte = match self.walk(a, true) {
                Some(pte) => pte,
                None => {
                    return false;
                }
            };
            if pte_is_valid(unsafe { *pte }) {
                printk!("vm_map: remap va {:#x}", a);
                return false;
            }
            unsafe {
                *pte = pa_to_pte(pa, flags | PTE_V);
            }
            if a == last {
                break;
            }
            a += PGSIZE;
            pa += PGSIZE;
        }
        true
    }
    fn unmap(&self, va: VirtAddr, len: usize, free: bool) -> bool {
        let start = align_down(va);
        let end = align_up(va + len);
        let mut a = start;
        let last = end - PGSIZE;
        while a <= last {
            let pte = match self.walk(a, false) {
                Some(pte) => pte,
                None => {
                    return false;
                }
            };
            let pa = pte_to_pa(unsafe { *pte });
            if !pte_is_valid(unsafe { *pte }) {
                printk!("vm_unmap: not mapped va {:#x}", a);
                return false;
            }
            if pte_check(unsafe { *pte }) {
                printk!("vm_unmap: pte points to a page table va {:#x}", a);
                return false;
            }
            if free {
                pmem_free(pa, true);
            }
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
        Some(pte) => pte,
        None => {
            panic!("vm_getpte: failed to get PTE for VA {:#x}", va);
        }
    }
}

pub fn vm_mappages(table: &PageTable, va: VirtAddr, size: usize, pa: PhysAddr, perm: usize) {
    match table.map(va, pa, size, perm | PTE_V) {
        true => {}
        false => {
            panic!("vm_mappages: failed to map VA {:#x} to PA {:#x}", va, pa);
        }
    }
}

pub fn vm_unmappages(table: &PageTable, va: VirtAddr, size: usize, free: bool) {
    match table.unmap(va, size, free) {
        true => {}
        false => {
            panic!("vm_unmappages: failed to unmap VA {:#x}", va);
        }
    }
}

#[cfg(feature = "tests")]
pub fn vm_print(table: &PageTable) {
    let level2 = table;
    let mut pte: Pte;
    printk!("Page Table at {:p}\n", level2);
    for i in 0..PGNUM {
        use crate::mem::pte::pte_to_pa;

        pte = level2.entries[i];
        if !pte_is_valid(pte) {
            continue;
        }
        if !pte_check(pte) {
            panic!("vm_print: invalid PTE at level 2 {}", pte);
        }
        let level1 = pte_to_pa(pte) as *const PageTable;
        printk!("   L2 {:3} PTE {:016x} -> {:p}\n", i, pte, level1);
        for j in 0..PGNUM {
            let level0 = unsafe { (*level1).entries[j] };
            if !pte_is_valid(level0) {
                continue;
            }
            if !pte_check(level0) {
                panic!("vm_print: invalid PTE at level 1 {}", level0);
            }
            let level0_pa = pte_to_pa(level0) as *const PageTable;
            printk!("       L1 {:3} PTE {:016x} -> {:p}\n", j, level0, level0_pa);
            for k in 0..PGNUM {
                pte = unsafe { (*level0_pa).entries[k] };
                if !pte_is_valid(pte) {
                    continue;
                }
                if !pte_check(pte) {
                    panic!("vm_print: invalid PTE at level 0 {}", pte);
                }
                let pa = pte_to_pa(pte);
                let va = (i << 30) | (j << 21) | (k << 12);
                printk!(
                    "           L0 {:3} PTE {:016x} -> {:p} VA {:p}\n",
                    k,
                    pte,
                    pa as *const u8,
                    va as *const u8
                );
            }
        }
    }
}

#[inline(always)]
fn make_satp(ppn: usize) -> usize {
    SATP_SV39 | ppn
}

pub fn init_kernel_vm() {}
pub fn switch_to_kernel_vm() {
    let root_pa = (&KERNEL_PAGE_TABLE as *const PageTable) as usize;

    unsafe {
        asm!("csrw satp, {}", in(reg) make_satp(root_pa >> 12));
        asm!("sfence.vma zero, zero");
    }
}
