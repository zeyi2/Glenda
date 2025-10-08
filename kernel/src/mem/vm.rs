use core::arch::asm;
use core::panic;

use super::PGNUM;
use super::addr::{PhysAddr, VirtAddr};
use super::pte::{PTE_V, Pte};
use super::pte::{pte_check, pte_is_valid};
use crate::printk;

const SATP_SV39: usize = 8 << 60;

#[repr(C)]
#[derive(Clone, Copy)]
struct PageTable {
    entries: [Pte; PGNUM],
}

unsafe impl Sync for PageTable {}

static KERNEL_PAGE_TABLE: PageTable = PageTable { entries: [0; PGNUM] };

impl PageTable {
    fn map(&self, va: VirtAddr, pa: PhysAddr, flags: usize) -> bool {
        false
    }
    fn unmap(&self, va: VirtAddr, len: usize, free: bool) -> bool {
        false
    }
}

fn vm_walk(table: &PageTable, va: VirtAddr, alloc: bool) -> Option<*mut Pte> {
    None
}

pub fn vm_getpte(table: &PageTable, va: VirtAddr) -> *mut Pte {
    match vm_walk(table, va, false) {
        Some(pte) => pte,
        None => {
            panic!("vm_getpte: failed to get PTE for VA {:#x}", va);
        }
    }
}

pub fn vm_mappages(table: &PageTable, va: VirtAddr, size: usize, pa: PhysAddr, perm: usize) {
    match table.map(va, pa, perm | PTE_V) {
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
