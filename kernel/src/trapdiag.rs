// Debugging Utils
#![allow(dead_code)]

use core::ptr::read_volatile;

use riscv::register::{satp, scause, sepc, stval, sstatus};

use crate::mem::pte::{
    Pte, pte_is_valid, pte_is_leaf, pte_is_table, pte_get_flags, pte_to_pa,
};
use crate::mem::{PGNUM, PGSIZE};
use crate::mem::addr::{vpn, VirtAddr};

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TrapRec {
    pub scause_bits: usize,
    pub code: usize,
    pub sepc: usize,
    pub stval: usize,
    pub sstatus: usize,
    pub satp: usize,
    pub sp: usize,
}

#[unsafe(no_mangle)]
pub static mut LAST_TRAP: TrapRec = TrapRec {
    scause_bits: 0, code: 0, sepc: 0, stval: 0, sstatus: 0, satp: 0, sp: 0,
};

#[inline(always)]
pub fn read_sp() -> usize {
    let sp: usize;
    unsafe { core::arch::asm!("mv {}, sp", out(reg) sp); }
    sp
}

pub struct PtLookup {
    pub level_hit: Option<u8>,
    pub pte: Option<Pte>,
    pub pa: Option<usize>,
    pub flags: Option<usize>,
}

pub fn sv39_lookup_va(va: VirtAddr) -> PtLookup {
    let mut out = PtLookup { level_hit: None, pte: None, pa: None, flags: None };

    let satp_val = satp::read().bits();
    let root_ppn = satp_val & ((1usize << 44) - 1);
    if root_ppn == 0 { return out; }

    let mut table_pa = root_ppn << 12;
    unsafe { let _ = read_volatile(table_pa as *const u64); }

    let idx2 = vpn(va)[2];
    let pte2 = unsafe { *((table_pa + idx2 * core::mem::size_of::<Pte>()) as *const Pte) };
    if !pte_is_valid(pte2) { return out; }
    if pte_is_leaf(pte2) {
        out.level_hit = Some(2);
        out.pte = Some(pte2);
        out.pa = Some(pte_to_pa(pte2));
        out.flags = Some(pte_get_flags(pte2));
        return out;
    }
    assert!(pte_is_table(pte2));
    table_pa = pte_to_pa(pte2);

    let idx1 = vpn(va)[1];
    let pte1 = unsafe { *((table_pa + idx1 * core::mem::size_of::<Pte>()) as *const Pte) };
    if !pte_is_valid(pte1) { return out; }
    if pte_is_leaf(pte1) {
        out.level_hit = Some(1);
        out.pte = Some(pte1);
        out.pa = Some(pte_to_pa(pte1));
        out.flags = Some(pte_get_flags(pte1));
        return out;
    }
    assert!(pte_is_table(pte1));
    table_pa = pte_to_pa(pte1);

    let idx0 = vpn(va)[0];
    let pte0 = unsafe { *((table_pa + idx0 * core::mem::size_of::<Pte>()) as *const Pte) };
    if !pte_is_valid(pte0) { return out; }
    if !pte_is_leaf(pte0) { return out; }

    out.level_hit = Some(0);
    out.pte = Some(pte0);
    out.pa = Some(pte_to_pa(pte0));
    out.flags = Some(pte_get_flags(pte0));
    out
}

pub fn record_trap_only() {
    let sc = scause::read();
    unsafe {
        LAST_TRAP.scause_bits = sc.bits();
        LAST_TRAP.code = match sc.cause() {
            scause::Trap::Exception(e) => e as usize,
            scause::Trap::Interrupt(i) => (1usize << 63) | (i as usize),
        };
        LAST_TRAP.sepc = sepc::read();
        LAST_TRAP.stval = stval::read();
        LAST_TRAP.sstatus = sstatus::read().bits();
        LAST_TRAP.satp = satp::read().bits();
        LAST_TRAP.sp = read_sp();
    }
}

pub fn dump_last_trap() {
    use crate::printk;

    let rec = unsafe { LAST_TRAP };
    if rec.scause_bits == 0 { printk!("(no trap recorded)"); return; }

    printk!("TRAP rec: scause.bits={:#x} code={} sepc={:#018x} stval={:#018x} sstatus={:#x} satp={:#x} sp={:#018x}",
        rec.scause_bits, rec.code, rec.sepc, rec.stval, rec.sstatus, rec.satp, rec.sp);

    let look_sepc = sv39_lookup_va(rec.sepc);
    match (look_sepc.level_hit, look_sepc.pte, look_sepc.flags, look_sepc.pa) {
        (Some(lv), Some(pte), Some(flags), Some(pa)) => {
            printk!("sepc mapping: level={} pte={:#x} flags={:#x} -> PA={:#x}", lv, pte, flags, pa);
        }
        _ => {
            printk!("sepc mapping: <not mapped / invalid structure>");
        }
    }
    let look_stval = sv39_lookup_va(rec.stval);
    match (look_stval.level_hit, look_stval.pte, look_stval.flags, look_stval.pa) {
        (Some(lv), Some(pte), Some(flags), Some(pa)) => {
            printk!("stval mapping: level={} pte={:#x} flags={:#x} -> PA={:#x}", lv, pte, flags, pa);
        }
        _ => {
            printk!("stval mapping: <not mapped / invalid structure>");
        }
    }

    let look_sp = sv39_lookup_va(rec.sp);
    match (look_sp.level_hit, look_sp.pte, look_sp.flags, look_sp.pa) {
        (Some(lv), Some(pte), Some(flags), Some(pa)) => {
            printk!("sp mapping:    level={} pte={:#x} flags={:#x} -> PA={:#x}", lv, pte, flags, pa);
        }
        _ => {
            printk!("sp mapping:    <not mapped / invalid structure>");
        }
    }
}
