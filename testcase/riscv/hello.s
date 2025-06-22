  .data
  .globl iivtewr
iivtewr:
  .word 0

  .text
  .globl main
main:
  addi sp, sp, -16
  sw ra, 12(sp)
  la t1, iivtewr
  lw t1, 0(t1)
  li s1, 1
  add t2, t1, s1
  mv a0, t2
  lw ra, 12(sp)
  addi sp, sp, 16
  ret

