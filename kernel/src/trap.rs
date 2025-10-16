use riscv::asm::wfi;
use riscv::register::scause::Trap;
use crate::trapdiag;

#[unsafe(no_mangle)]
pub extern "C" fn glenda_trap() -> ! {
    let sc = riscv::register::scause::read();
    match sc.cause() {
        Trap::Exception(_) | Trap::Interrupt(_) => {
            trapdiag::record_trap_only();
            loop { wfi(); }
        }
    }
}
