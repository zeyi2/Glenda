#![allow(dead_code)]

use core::cell::OnceCell;
use core::cmp;
use core::ptr::{self, NonNull, addr_of_mut};

use spin::Mutex;

use crate::dtb;
use crate::mem::addr::PhysAddr;
use crate::printk;
use crate::printk::{ANSI_BLUE, ANSI_RED, ANSI_RESET};

pub const PGSIZE: usize = 4096;

unsafe extern "C" {
    static mut __bss_end: u8;
}

#[repr(C)]
struct FreePage {
    next: Option<NonNull<FreePage>>,
}

#[derive(Clone, Copy)]
struct RegionInner {
    head: Option<NonNull<FreePage>>,
    allocable: usize,
}

#[derive(Debug, Clone, Copy)]
struct RegionBounds {
    begin: PhysAddr,
    end: PhysAddr,
}

struct AllocRegion {
    bounds: OnceCell<RegionBounds>,
    inner: Mutex<RegionInner>,
}

unsafe impl Sync for AllocRegion {}

impl AllocRegion {
    const fn new() -> Self {
        Self {
            bounds: OnceCell::new(),
            inner: Mutex::new(RegionInner { head: None, allocable: 0 }),
        }
    }

    unsafe fn init(&self, begin: PhysAddr, end: PhysAddr) {
        let begin_aligned = begin.align_up();
        let end_aligned = end.align_down();

        let mut head: Option<NonNull<FreePage>> = None;
        let mut count = 0usize;
        let mut current = begin_aligned.as_usize();
        let end_val = end_aligned.as_usize();

        while current + PGSIZE <= end_val {
            let page = current as *mut FreePage;
            unsafe {
                (*page).next = head;
            }
            head = NonNull::new(page);
            count += 1;
            current += PGSIZE;
        }

        self.bounds
            .set(RegionBounds { begin: begin_aligned, end: end_aligned })
            .expect("AllocRegion::init called twice");

        *self.inner.lock() = RegionInner { head, allocable: count };
    }

    fn region_info(&self) -> RegionInfo {
        let b = *self.bounds.get().expect("region not initialized");
        let allocable = self.inner.lock().allocable;
        RegionInfo { begin: b.begin, end: b.end, allocable }
    }

    fn allocate(&self) -> Option<PhysAddr> {
        let head_ptr = {
            let mut inner = self.inner.lock();
            let head = inner.head?;
            let next = unsafe { (*head.as_ptr()).next };
            inner.head = next;
            inner.allocable = inner.allocable.saturating_sub(1);
            head
        };

        let p = head_ptr.as_ptr() as usize;
        unsafe { ptr::write_bytes(p as *mut u8, 0, PGSIZE) };
        Some(PhysAddr::new(p))
    }

    fn free(&self, addr: PhysAddr) {
        let b = *self.bounds.get().expect("region not initialized");
        let pa = addr;
        if pa < b.begin || pa >= b.end || pa.page_offset() != 0 {
            panic!(
                "pmem_free: address {:#x} out of bounds [{:#x}, {:#x}]",
                pa.as_usize(),
                b.begin.as_usize(),
                b.end.as_usize()
            );
        }

        let mut inner = self.inner.lock();
        unsafe {
            ptr::write_bytes(pa.as_usize() as *mut u8, 0, PGSIZE);
            let page = pa.as_usize() as *mut FreePage;
            (*page).next = inner.head;
            inner.head = NonNull::new(page);
        }
        inner.allocable += 1;
    }
}

static KERNEL_REGION: AllocRegion = AllocRegion::new();
static USER_REGION: AllocRegion = AllocRegion::new();

#[derive(Clone, Copy, Debug)]
pub struct RegionInfo {
    pub begin: PhysAddr,
    pub end: PhysAddr,
    pub allocable: usize,
}

pub fn pmem_alloc(for_kernel: bool) -> PhysAddr {
    match allocate_page(for_kernel) {
        Some(pa) => pa,
        None => {
            if for_kernel {
                panic!("pmem_alloc: kernel region exhausted");
            } else {
                panic!("pmem_alloc: user region exhausted");
            }
        }
    }
}

#[cfg(feature = "tests")]
pub fn pmem_try_alloc(for_kernel: bool) -> Option<PhysAddr> {
    allocate_page(for_kernel)
}

pub fn pmem_free(addr: PhysAddr, for_kernel: bool) {
    region(for_kernel).free(addr);
}

pub fn kernel_region_info() -> RegionInfo {
    KERNEL_REGION.region_info()
}

pub fn user_region_info() -> RegionInfo {
    USER_REGION.region_info()
}

fn allocate_page(for_kernel: bool) -> Option<PhysAddr> {
    region(for_kernel).allocate()
}

fn region(for_kernel: bool) -> &'static AllocRegion {
    if for_kernel { &KERNEL_REGION } else { &USER_REGION }
}

pub fn initialize_regions() {
    let kernel_end = PhysAddr::new(addr_of_mut!(__bss_end) as usize).align_up();

    let mem_range = dtb::memory_range()
        .unwrap_or_else(|| dtb::MemoryRange { start: 0x8000_0000, size: 128 * 1024 * 1024 });
    let mem_start = PhysAddr::new(mem_range.start);
    let mem_end = PhysAddr::new(mem_range.start + mem_range.size);

    if kernel_end.as_usize() >= mem_end.as_usize() {
        printk!(
            "{}PMEM init failed{}: kernel end {:#x} beyond memory end {:#x}",
            ANSI_RED,
            ANSI_RESET,
            kernel_end.as_usize(),
            mem_end.as_usize()
        );
        panic!("pmem_init: kernel overlaps physical memory end");
    }

    let alloc_begin = PhysAddr::new(cmp::max(kernel_end.as_usize(), mem_start.as_usize()));
    let alloc_end = mem_end;
    let total_free = alloc_end.as_usize().saturating_sub(alloc_begin.as_usize());

    let mut kernel_split = PhysAddr::new(alloc_begin.as_usize() + total_free / 2).align_up();
    if kernel_split > alloc_end {
        kernel_split = alloc_end;
    }
    if kernel_split < alloc_begin {
        kernel_split = alloc_begin;
    }

    unsafe {
        KERNEL_REGION.init(alloc_begin, kernel_split);
        USER_REGION.init(kernel_split, alloc_end);
    }

    let k = KERNEL_REGION.region_info();
    let u = USER_REGION.region_info();

    printk!(
        "{}PMEM initialized{}: kernel [{:#x}, {:#x}) -> {} pages, user [{:#x}, {:#x}) -> {} pages",
        ANSI_BLUE,
        ANSI_RESET,
        k.begin.as_usize(),
        k.end.as_usize(),
        k.allocable,
        u.begin.as_usize(),
        u.end.as_usize(),
        u.allocable
    );
}
