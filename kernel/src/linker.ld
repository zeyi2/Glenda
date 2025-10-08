OUTPUT_ARCH(riscv)
ENTRY(_start)

SECTIONS
{
  /*
     QEMU Virt 的 RAM 从 0x80000000 开始,
     OpenSBI 会把内核放到 0x80200000
   */
  . = 0x80200000;

  .text : ALIGN(4) {
    PROVIDE(__text_start = .);
    KEEP(*(.text.start))
    *(.text .text.*)
    PROVIDE(__text_end = .);
  }

  .rodata : ALIGN(16) {
    PROVIDE(__rodata_start = .);
     *(.rodata .rodata.*)
    PROVIDE(__rodata_end = .);
  }

  .data : ALIGN(16) {
    PROVIDE(__data_start = .);
    *(.sdata .sdata.* .data .data.*)  
    PROVIDE(__data_end = .);
  }

  .bss : ALIGN(16) {
    PROVIDE(__bss_start = .);
    *(.sbss .sbss.* .bss .bss.* COMMON)
    PROVIDE(__bss_end = .);
  }

  . = ALIGN(16);
  PROVIDE(__alloc_start = .);
}
