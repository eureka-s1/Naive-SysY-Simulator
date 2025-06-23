.data
.text
.globl main
main:
  addi	sp, sp, -32
  sw	fp, 0(sp)
  sw	ra, 4(sp)
  addi	fp, sp, 0
main_main_0entry_0:
  li	t0, 4
  mv	a0, t0
  lw	ra, 4(sp)
  lw	fp, 0(sp)
  addi	sp, sp, 32
  ret
