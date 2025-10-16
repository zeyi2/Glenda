// kernel/src/uart_emerg.rs
#[inline(always)]
fn uart_base() -> usize {
    0x1000_0000
}

#[inline(always)]
pub fn emerg_putb(b: u8) {
    const LSR_OFF: usize = 5;
    const THR_OFF: usize = 0;
    const THRE: u8 = 0x20;
    unsafe {
        let lsr = (uart_base() + LSR_OFF) as *const u8;
        let thr = (uart_base() + THR_OFF) as *mut u8;
        while core::ptr::read_volatile(lsr) & THRE == 0 {}
        core::ptr::write_volatile(thr, b);
    }
}

pub fn emerg_puts(s: &str) {
    for &b in s.as_bytes() { emerg_putb(b); }
}

pub fn emerg_hex(mut x: usize) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    emerg_puts("0x");
    let mut started = false;
    for i in (0..(core::mem::size_of::<usize>()*2)).rev() {
        let nyb = (x >> (i*4)) & 0xF;
        if nyb != 0 || started || i == 0 { started = true; emerg_putb(HEX[nyb]); }
    }
}
