.data
  .globl global_0
global_0:
  .word 0
.text
.globl main
main:
  addi	sp, sp, -32
  sw	fp, 0(sp)
  sw	ra, 4(sp)
  addi	fp, sp, 0
main_main_0entry_0:
  la	t0, global_0
  lw	t1, 0(t0)
  sw	t1, 8(fp)
  lw	t0, 8(fp)
  li	t1, 1
  add	t2, t0, t1
  sw	t2, 12(fp)
  lw	t0, 12(fp)
  mv	a0, t0
  lw	ra, 4(sp)
  lw	fp, 0(sp)
  addi	sp, sp, 32
  ret
