#![allow(dead_code)]

use core::cell::OnceCell;
use core::ptr::{self, NonNull, addr_of_mut};

use spin::{Mutex, Once};

use super::PGSIZE;
use super::addr::{PhysAddr, align_down, align_up};
use crate::dtb;
use crate::mem::KERN_PAGES;
use crate::printk;

unsafe extern "C" {
    static mut __bss_end: u8;
    static mut __alloc_start: u8;
}

static PMEM_INIT: Once<()> = Once::new();

pub fn initialize_regions(hartid: usize) {
    PMEM_INIT.call_once(|| unsafe { _init_inner(hartid) });
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

    fn contains(&self, addr: PhysAddr) -> bool {
        if let Some(b) = self.bounds.get() { addr >= b.begin && addr < b.end } else { false }
    }

    unsafe fn init(&self, begin: PhysAddr, end: PhysAddr) {
        let begin_aligned = align_up(begin);
        let end_aligned = align_down(end);

        let mut head: Option<NonNull<FreePage>> = None;
        let mut count = 0usize;
        let mut current = begin_aligned;

        while current + PGSIZE <= end_aligned {
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

    fn allocate(&self) -> Option<*mut u8> {
        let head_ptr = {
            let mut inner = self.inner.lock();
            let head = inner.head?;
            let next = unsafe { (*head.as_ptr()).next };
            inner.head = next;
            inner.allocable = inner.allocable.saturating_sub(1);
            head
        };

        let p = head_ptr.as_ptr() as *mut u8;
        unsafe { ptr::write_bytes(p, 0, PGSIZE) };
        Some(p)
    }

    fn free(&self, addr: PhysAddr) {
        let b = *self.bounds.get().expect("region not initialized");
        if addr < b.begin || addr >= b.end || addr % PGSIZE != 0 {
            panic!("pmem_free: address {:#x} out of bounds [{:#x}, {:#x}]", addr, b.begin, b.end);
        }

        let mut inner = self.inner.lock();
        unsafe {
            //ptr::write_bytes(addr as *mut u8, 0, PGSIZE);
            let page = addr as *mut FreePage;
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

pub fn pmem_alloc(for_kernel: bool) -> *mut u8 {
    match allocate_page(for_kernel) {
        Some(ptr) => ptr,
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
pub fn pmem_try_alloc(for_kernel: bool) -> Option<*mut u8> {
    allocate_page(for_kernel)
}

pub fn pmem_free(addr: PhysAddr, _for_kernel: bool) {
    if KERNEL_REGION.contains(addr) {
        KERNEL_REGION.free(addr);
    } else if USER_REGION.contains(addr) {
        USER_REGION.free(addr);
    } else {
        panic!("pmem_free: address {:#x} out of all regions", addr);
    }
}

pub fn kernel_region_info() -> RegionInfo {
    KERNEL_REGION.region_info()
}

pub fn user_region_info() -> RegionInfo {
    USER_REGION.region_info()
}

fn allocate_page(for_kernel: bool) -> Option<*mut u8> {
    region(for_kernel).allocate()
}

fn region(for_kernel: bool) -> &'static AllocRegion {
    if for_kernel { &KERNEL_REGION } else { &USER_REGION }
}

unsafe fn _init_inner(hartid: usize) {
    let kernel_end = align_up(addr_of_mut!(__bss_end) as PhysAddr);

    let mem_range = dtb::memory_range()
        .unwrap_or_else(|| dtb::MemoryRange { start: 0x8000_0000, size: 128 * 1024 * 1024 });
    let mem_end = mem_range.start + mem_range.size;

    if kernel_end >= mem_end {
        panic!("pmem_init: kernel end {:#x} beyond memory end {:#x}", kernel_end, mem_end);
    }

    let alloc_begin = addr_of_mut!(__alloc_start) as PhysAddr;
    debug_assert!(alloc_begin >= align_up(addr_of_mut!(__bss_end) as PhysAddr));
    debug_assert_eq!(alloc_begin & (PGSIZE - 1), 0, "__alloc_start must be 4K-aligned");

    let alloc_end = mem_end;
    let total_free = alloc_end.saturating_sub(alloc_begin);
    printk!(
        "PMEM: physical memory [{:#x}, {:#x}) -> {} MiB, free [{:#x}, {:#x}) -> {} MiB",
        mem_range.start,
        mem_end,
        mem_range.size / (1024 * 1024),
        alloc_begin,
        alloc_end,
        total_free / (1024 * 1024)
    );

    let mut kernel_split = align_up(alloc_begin + KERN_PAGES * PGSIZE);
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
    debug_assert_eq!(k.begin & (PGSIZE - 1), 0);
    debug_assert_eq!(k.end & (PGSIZE - 1), 0);
    debug_assert_eq!(u.begin & (PGSIZE - 1), 0);
    debug_assert_eq!(u.end & (PGSIZE - 1), 0);

    printk!(
        "PMEM: Initialized kernel [{:#x}, {:#x}) -> {} pages, user [{:#x}, {:#x}) -> {} pages on hart {}",
        k.begin,
        k.end,
        k.allocable,
        u.begin,
        u.end,
        u.allocable,
        hartid
    );
}

#[inline]
pub fn kernel_pool_range() -> (PhysAddr, PhysAddr) {
    let info = KERNEL_REGION.region_info();
    (info.begin, info.end)
}
