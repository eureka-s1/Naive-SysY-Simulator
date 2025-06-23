# Naive-Sysy-Simulator Report

## 1.项目概述

- 本项目旨在实现一个从SysY语言到RISC-V指令集的转换工具，并能够生成可执行文件。项目的核心目标是通过中间表示（IR）和目标代码生成的过程，最终实现从高级语言到机器代码的完整转换链条。
- Rust语言的优秀类型机制一定程度上简化了代码编写，同时与语法分析的树形结构相契合

## 2.模块构成

项目的核心实现位于src目录下，具体模块划分如下：

- **Lexer & Parser**：基于 LALRPOP 的 `sysy.lalrpop` 语法文件，负责词法和语法分析，生成 AST。
- **Irgen：** 中间代码生成模块，将SysY源代码转化为Koopa IR。
  - **AST 模块**：定义了 `CompUnit`、`Exp`、`Stmt`、`Decl` 等语法树节点数据结构。
  - **Koopa crate**：利用koopa的crate将 AST 转换为 Koopa IR 在内存形式上的layout。
- **Codegen：** 目标代码生成模块，将中间代码（Koopa IR）转化为RISC-V汇编代码。
  - **层次结构**：按照`Program`,`Function`,`Value`的层次遍历KoopaIR语句，最后依照`ValueKind`进行pattern matching，匹配不同语句逻辑。
- **Simulator：** 将RISC-V汇编代码转化为可执行文件的模块。

- **驱动程序**（`main.rs`）：解析命令行参数，根据 `-koopa` ,`-riscv`,`-sim` 模式调用对应的生成函数，并写入输出文件。
  


## 3.编码细节

### 数据结构

- 最核心的数据结构是抽象语法树（AST）。在实现过程中，设计了 struct CompUnit 用来表
示编译单元，它是整个 SysY 程序树的根。
    ```
    pub struct CompUnit {
    pub decls: Vec<Decl>,
    pub funcs: Vec<FuncDef>,
    }
    ```
- 依次向下构造 AST 的结构，对于每个组成部分，都给它实现 `generate trait` ，使其在结构上具有一定的统一性
- 在 `if...else... `语句方面，因涉及二义性问题，设计了 `MatchedStmt` 结构明确区分条件分支。
- 除此之外，为了代码编写的便利性，在编译器的前端和后端部分都各有一个` Env` 的数据结构。设计 `Env` 的目的是存储全局信息的符号表等，如全局变量、函数声明，方便各模块访问.
    ```
    pub struct Env<'p> {
        pub global_vars: HashMap<String, Type>,
        pub func_decls: HashMap<String, FuncType>,
        pub program: Program,
        ...
    }
    ```

### 符号表管理

- 符号表采用分层结构处理变量作用域。进入新作用域时，创建新符号表层，退出时销毁。查找符号时从当前层逐级向上。
- 全局变量存于最底层，局部变量按作用域分层存储。如函数参数和局部变量在函数符
号表层，Env 结构体在最外层存储全局变量 global_var ；嵌套块的局部变量在独立层，保证作用域隔离与正确查找。

### 寄存器分配策略

- 采用最简单的栈式分配：所有局部变量与中间值统一分配到栈帧中，不做全局寄存器分配。使用固定的临时寄存器 `t0`、`t1`、`t2` 进行计算，中间结果立即溢出到栈，有效简化了寄存器分配逻辑。
- 在进入函数体前根据use_by关系计算出大致所需的空间，并且维护一个`slot_offsets`哈希表为所有本地变量匹配偏移量。

### 控制流

- `if`语句处理有/无 `else` 情况，生成条件跳转标签，若无 `else` 则在 `false` 分支跳至后继。
- `while` 语句生成循环入口与退出标签，维护 `loop_stack` 支持 `break`/`continue`。

### 函数调用

- 函数栈帧大小向 16 字节对齐，保证调用约定和性能。
- 前 8 个形参使用 a0–a7 寄存器，超过部分通过栈上传递；返回值通过 a0 返回。

### 数组

- 初始化遇到 initlist 就递归处理，递归大小规模由对齐处理。本质上，只需要处
理对齐到哪一维即可。
- 数组传参时：
  - 若调用时使用的维度个数等于初始化时知道的维度个数，则其为值，补上 load 指令
  - 若调用时使用的维度个数小于初始化时知道的维度个数，则其为指针，补上getelemptr 指令
  
## 4.测试与运行

程序存放在 testcase/c 目录下
### 前段代码生成
```
cargo run -- -koopa hello.c -o hello.koopa 
```
生成 Koopa IR 代码
```
cargo run -- -riscv hello.c -o hello.s 
```
生成 RISC-V 代码
运行展示：
<video controls src="hello.c - Naive-SysY-Simulator - Visual Studio Code 2025-06-23 21-04-34.mp4" title="Title"></video>

### 后端代码运行
```
cargo run -- -sim hello.c -o hello.out 
```
生成 可执行文件
这里在图形化窗口Pipeline Simulator上实现了类似gdb的测试操作，可以在输入窗口输入指令，按钮运行。
可以使用的指令包括：
```
Available commands:
  c          - Continue execution
  q          - Quit the simulator
  si [N]     - Single step execution (N times, default 1)
  info r     - Print register state to output
  x N ADDR   - Examine memory at address ADDR, N words
              (ADDR format: 0x1234 or 1234)
  help       - Print this help information
```
运行展示：
<video controls src="Naive-SysY-Simulator - Visual Studio Code 2025-06-23 21-10-22.mp4" title="Title"></video>

## 5.项目分工

陆奕涵实现了目标代码生成和Pipeline Simulator的图形化和除sim部分外的实验报告

## 6.出勤情况
陆奕涵 10/14
生活照：![alt text](2de430440b5bfe49b0cb95a1cb82b86.jpg)

