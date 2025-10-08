use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, Ordering};

use super::barrier::FINAL_BARRIER;
use crate::printk;
use crate::printk::{ANSI_GREEN, ANSI_RESET};

// 标志最终 PASS 已打印
static FINAL_DONE: AtomicBool = AtomicBool::new(false);

pub fn run_tests(hartid: usize) {
    run_spinlock_tests(hartid);
    run_printk_tests(hartid);
    run_pmem_tests(hartid);
    run_vm_tests(hartid);

    // 最终同步：所有测试结束后再统一进入 main loop
    // 初始化（任意先到可执行）；如果已经 init 则忽略
    FINAL_BARRIER.ensure_inited(crate::dtb::hart_count());
    FINAL_BARRIER.wait_start();
    let last = FINAL_BARRIER.finish_and_last();
    if last {
        printk!(
            "{}All tests completed across {} harts{}",
            ANSI_GREEN,
            FINAL_BARRIER.total(),
            ANSI_RESET
        );
        FINAL_DONE.store(true, Ordering::Release);
    } else {
        while !FINAL_DONE.load(Ordering::Acquire) {
            spin_loop();
        }
    }
    // 在 test() 返回前所有 hart 打印进入主循环提示，避免因为后续输出被截断而误判未进入
    printk!("Hart {} tests done, returning to main loop", hartid);
}

fn run_spinlock_tests(hartid: usize) {
    super::spinlock::run(hartid);
}
fn run_pmem_tests(hartid: usize) {
    super::pmem::run(hartid);
}

fn run_vm_tests(hartid: usize) {
    super::vm::run(hartid);
}

fn run_printk_tests(hartid: usize) {
    super::printk::run(hartid);
}
