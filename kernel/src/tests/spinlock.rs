#![allow(dead_code)]

use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::dtb;
use crate::printk;
use crate::printk::{ANSI_BLUE, ANSI_GREEN, ANSI_RESET, ANSI_YELLOW};
use spin::Mutex;

const INCREMENTS_PER_HART: usize = 16;
static TEST_LOCK: Mutex<()> = Mutex::new(());
static GLOBAL_COUNTER: AtomicUsize = AtomicUsize::new(0);
static PARTICIPANTS: AtomicUsize = AtomicUsize::new(0);
static START_TEST: AtomicBool = AtomicBool::new(false);
static HARTS_FINISHED: AtomicUsize = AtomicUsize::new(0);

fn spinlock_test(hartid: usize, harts_under_test: usize) -> usize {
    if hartid >= harts_under_test {
        return 0;
    }

    let ready = PARTICIPANTS.fetch_add(1, Ordering::SeqCst) + 1;
    if ready == harts_under_test {
        printk!(
            "{}All {} harts ready. Starting spinlock test{}",
            ANSI_BLUE,
            harts_under_test,
            ANSI_RESET
        );
        START_TEST.store(true, Ordering::SeqCst);
    } else {
        while !START_TEST.load(Ordering::SeqCst) {
            spin_loop();
        }
    }

    for iter in 0..INCREMENTS_PER_HART {
        {
            let _guard = TEST_LOCK.lock();
            let value_before = GLOBAL_COUNTER.load(Ordering::Relaxed);
            driver_uart::print!(
                "[hart {}] iter {} -> counter {}\n",
                hartid,
                iter,
                value_before + 1
            );
            GLOBAL_COUNTER.store(value_before + 1, Ordering::Relaxed);
        }
    }
    HARTS_FINISHED.fetch_add(1, Ordering::SeqCst) + 1
}

pub fn run(hartid: usize) {
    printk!("{}[TEST]{} Spinlock test start (hart {})", ANSI_YELLOW, ANSI_RESET, hartid);
    let harts_under_test = dtb::hart_count();
    let result = spinlock_test(hartid, harts_under_test);
    if result == 0 {
        printk!("{}hart {} idle{} (not part of spinlock test)", ANSI_YELLOW, hartid, ANSI_RESET);
        return;
    }

    if result == harts_under_test {
        let expected = harts_under_test * INCREMENTS_PER_HART;
        let final_value = GLOBAL_COUNTER.load(Ordering::SeqCst);
        assert_eq!(
            final_value, expected,
            "Spinlock test: counter mismatch (expected {}, got {})",
            expected, final_value
        );
        printk!(
            "{}[PASS]{} Spinlock test: counter reached {}",
            ANSI_GREEN,
            ANSI_RESET,
            final_value
        );
    }
}
