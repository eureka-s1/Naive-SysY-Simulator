# Naive-SysY-Simulator
Naive SysY Compiler and RISC-V Simulator


### 使用说明
程序存放在 `testcase/c` 目录下

`cargo run -- -koopa hello.c -o hello.koopa` 来生成 Koopa IR 代码

`cargo run -- -riscv hello.c -o hello.s` 来生成 RISC-V 代码


## 模块构成 

### 中间代码生成

### 目标代码生成

### RISCV 模拟器

`apt install gcc-riscv64-unknown-elf` 安装工具链


`riscv64-unknown-elf-gcc -march=rv64gc -mabi=lp64 -T build/scripts/linker.ld -Wl,--gc-sections -nostdlib -o example.elf example.s`

例如
`riscv64-unknown-elf-gcc -march=rv64gc -mabi=lp64 -T build/scripts/linker.ld -Wl,--gc-sections -nostdlib -o testcase/elf/exp.elf testcase/riscv/exp.s`

-nostdlib：不链接标准库

-T link.ld 强制使用指定内存布局

-Wl,--gc-sections 优化生成文件大小

生成的 example.elf 可直接用于模拟器。

