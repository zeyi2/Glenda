use core::arch::asm;
use core::panic;

use super::PGNUM;
use super::addr::{PhysAddr, VirtAddr};
use super::pte::{PTE_V, Pte};

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
pub fn vm_print(table: &PageTable) {}

#[inline(always)]
fn make_satp(ppn: usize) -> usize {
    SATP_SV39 | ppn
}

pub fn init_kernel_vm() {}
pub fn switch_to_kernel_vm() {
    unsafe {
        asm!("csrw satp, {}", in(reg) make_satp((&KERNEL_PAGE_TABLE as *const PageTable) as usize >> 12));
        asm!("sfence.vma zero, zero");
    }
}
