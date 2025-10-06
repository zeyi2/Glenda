use crate::mem::addr::{PhysAddr, VirtAddr};
use crate::mem::pmem::{PGSIZE, pmem_alloc};
use crate::mem::pte::{PTE_R, PTE_W, PTE_X, PageTable};
use crate::mem::vm::{vm_mappages, vm_print, vm_unmappages};
use crate::printk;
use crate::printk::{ANSI_GREEN, ANSI_RED, ANSI_RESET, ANSI_YELLOW};

// Sv39 虚拟地址空间最大值（512GB）
const VA_MAX: usize = 1 << 39;

pub fn run(hartid: usize) {
    // 只在主核上运行测试
    if hartid != 0 {
        return;
    }

    printk!("{}[TEST]{} VM: Starting virtual memory tests", ANSI_YELLOW, ANSI_RESET);

    test_vm_basic();
    test_vm_remap_and_unmap();

    printk!("{}[PASS]{} VM: All tests passed", ANSI_GREEN, ANSI_RESET);
}

/// 测试 1: 基本页表映射
fn test_vm_basic() {
    printk!("{}[TEST]{} VM: basic mapping test", ANSI_YELLOW, ANSI_RESET);

    // 分配测试页表
    let test_pgtbl_pa = pmem_alloc(true);
    let test_pgtbl = unsafe { &mut *(test_pgtbl_pa.as_usize() as *mut PageTable) };
    *test_pgtbl = PageTable::new();

    // 分配 5 个物理页用于测试
    let mut mem = [PhysAddr(0); 5];
    for i in 0..5 {
        mem[i] = pmem_alloc(false);
    }

    printk!("  Allocated test page table at PA {:#x}", test_pgtbl_pa.as_usize());
    printk!("  Allocated 5 physical pages for testing");

    // 执行各种映射测试
    // 测试 1: 映射到地址 0
    vm_mappages(test_pgtbl, VirtAddr(0), mem[0], PGSIZE, PTE_R);
    printk!("  Mapped VA 0x0 -> PA {:#x} (R)", mem[0].as_usize());

    // 测试 2: 映射到跨页表的地址（PGSIZE * 10）
    vm_mappages(test_pgtbl, VirtAddr(PGSIZE * 10), mem[1], PGSIZE / 2, PTE_R | PTE_W);
    printk!(
        "  Mapped VA {:#x} -> PA {:#x} (RW, {} bytes)",
        PGSIZE * 10,
        mem[1].as_usize(),
        PGSIZE / 2
    );

    // 测试 3: 映射到更大的虚拟地址（PGSIZE * 512）
    vm_mappages(test_pgtbl, VirtAddr(PGSIZE * 512), mem[2], PGSIZE - 1, PTE_R | PTE_X);
    printk!(
        "  Mapped VA {:#x} -> PA {:#x} (RX, {} bytes)",
        PGSIZE * 512,
        mem[2].as_usize(),
        PGSIZE - 1
    );

    // 测试 4: 映射到跨二级页表的地址（PGSIZE * 512 * 512）
    vm_mappages(test_pgtbl, VirtAddr(PGSIZE * 512 * 512), mem[3], PGSIZE, PTE_R | PTE_X);
    printk!("  Mapped VA {:#x} -> PA {:#x} (RX)", PGSIZE * 512 * 512, mem[3].as_usize());

    // 测试 5: 映射到虚拟地址空间最高处
    vm_mappages(test_pgtbl, VirtAddr(VA_MAX - PGSIZE), mem[4], PGSIZE, PTE_W);
    printk!("  Mapped VA {:#x} -> PA {:#x} (W)", VA_MAX - PGSIZE, mem[4].as_usize());

    printk!("\n  Page table dump:");
    vm_print(test_pgtbl, 0, 0);
    printk!("");

    printk!("{}[PASS]{} VM: basic mapping test completed", ANSI_GREEN, ANSI_RESET);
}

/// 测试 2: 重新映射和取消映射
fn test_vm_remap_and_unmap() {
    printk!("{}[TEST]{} VM: remap and unmap test", ANSI_YELLOW, ANSI_RESET);

    // 分配测试页表
    let test_pgtbl_pa = pmem_alloc(true);
    let test_pgtbl = unsafe { &mut *(test_pgtbl_pa.as_usize() as *mut PageTable) };
    *test_pgtbl = PageTable::new();

    // 分配 5 个物理页用于测试
    let mut mem = [PhysAddr(0); 5];
    for i in 0..5 {
        mem[i] = pmem_alloc(false);
    }

    // 首先建立初始映射
    vm_mappages(test_pgtbl, VirtAddr(0), mem[0], PGSIZE, PTE_R);
    vm_mappages(test_pgtbl, VirtAddr(PGSIZE * 10), mem[1], PGSIZE, PTE_R | PTE_W);
    vm_mappages(test_pgtbl, VirtAddr(PGSIZE * 512), mem[2], PGSIZE, PTE_R | PTE_X);

    printk!("  Initial mappings:");
    printk!("    VA 0x0 -> PA {:#x} (R)", mem[0].as_usize());
    printk!("    VA {:#x} -> PA {:#x} (RW)", PGSIZE * 10, mem[1].as_usize());
    printk!("    VA {:#x} -> PA {:#x} (RX)", PGSIZE * 512, mem[2].as_usize());

    // 测试重新映射：改变地址 0 的权限
    printk!("\n  Remapping VA 0x0 with different permissions (W only)");
    vm_mappages(test_pgtbl, VirtAddr(0), mem[0], PGSIZE, PTE_W);

    // 测试取消映射
    printk!("  Unmapping VA {:#x}", PGSIZE * 10);
    vm_unmappages(test_pgtbl, VirtAddr(PGSIZE * 10), PGSIZE, true);

    printk!("  Unmapping VA {:#x}", PGSIZE * 512);
    vm_unmappages(test_pgtbl, VirtAddr(PGSIZE * 512), PGSIZE, true);

    printk!("\n  Page table dump after modifications:");
    vm_print(test_pgtbl, 0, 0);
    printk!("");

    printk!("{}[PASS]{} VM: remap and unmap test completed", ANSI_GREEN, ANSI_RESET);
}
