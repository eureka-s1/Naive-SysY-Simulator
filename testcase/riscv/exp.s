.globl _trm_init
.globl _start
.globl cngpjmm
.globl main
.section .data
cngpjmm:
  .word 0

.section .text
_start:
  la sp, stack_top        
  jal _trm_init
_trm_init:
  addi sp, sp, -16
  sd ra, 8(sp)
  jal main
  ebreak
main:
  addi sp, sp, -16
  sw ra, 12(sp)
  la t1, cngpjmm
  lw t1, 0(t1)
  li s1, 1
  add t2, t1, s1
  mv a0, t2
  lw ra, 12(sp)
  addi sp, sp, 16
  ret

.section .bss
.align 4
stack_bottom:
  .skip 4096
stack_top:
