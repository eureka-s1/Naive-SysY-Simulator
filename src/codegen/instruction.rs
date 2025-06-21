type Imm12 = i32;
type Imm32 = i32;
pub type Reg = &'static str;

// | `beqz/bnez rs, label`      | 如果 `rs` 寄存器的值 eq/neq 0, 则转移到目标 `label`        |
// | `j label`                  | 无条件转移到目标 `label`                              |
// | `call label`               | 将后一条指令的地址存入 `ra` 寄存器, 并无条件转移到目标 `label`       |
// | `ret`                      | 无条件转移到 `ra` 寄存器中保存的地址处                        |
// | `lw rs, imm12(rd)`         | `R[rs] = Mem32[R[rd] + imm12]`                |
// | `sw rs2, imm12(rs1)`       | `Mem32[R[rs1] + imm12] = R[rs2]`              |
// | `add rd, rs1, rs2`         | `R[rd] = R[rs1] + R[rs2]`                     |
// | `addi rd, rs1, imm12`      | `R[rd] = R[rs1] + imm12`                      |
// | `sub rd, rs1, rs2`         | `R[rd] = R[rs1] - R[rs2]`                     |
// | `slt/sgt rd, rs1, rs2`     | `R[rd] = R[rs1] < R[rs2] / R[rs1] > R[rs2]`   |
// | `seqz/snez rd, rs`         | `R[rd] = R[rs1] == R[rs2] / R[rs1] != R[rs2]` |
// | `xor rd, rs1, rs2`         | `R[rd] = R[rs1] ^ R[rs2]`                     |
// | `xori rd, rs1, imm12`      | `R[rd] = R[rs1] ^ imm12`                      |
// | `or rd, rs1, rs2`          | `R[rd] = R[rs1] \| R[rs2]`                    |
// | `ori rd, rs1, imm12`       | `R[rd] = R[rs1] \| imm12`                     |
// | `and rd, rs1, rs2`         | `R[rd] = R[rs1] & R[rs2]`                     |
// | `andi rd, rs1, imm12`      | `R[rd] = R[rs1] & imm12`                      |
// | `sll/srl/sra rd, rs1, rs2` | `R[rd] = R[rs1] <</>>/>>> R[rs2]`             |
// | `mul/div/rem rd, rs1, rs2` | `R[rd] = R[rs1] */div/% R[rs2]`               |
// | `li rd, imm32`             | `R[rd] = imm32`                               |
// | `la rd, label`             | 将标号 `label` 的绝对地址加载到寄存器 `rd` 中                |
// | `mv rd, rs`                | 将寄存器 `rs` 的值复制到寄存器 `rd`                       |
#[derive(Debug, Clone)]
pub enum Inst {
    Beqz { rs: Reg, label: String },
    Bnez { rs: Reg, label: String },
    J { label: String },
    Call { label: String },
    Ret,
    Lw { rd: Reg, imm12: i32, rs: Reg },
    Sw { rs: Reg, imm12: i32, rd: Reg },
    Add { rd: Reg, rs1: Reg, rs2: Reg },
    Addi { rd: Reg, rs: Reg, imm12: i32 },
    Sub { rd: Reg, rs1: Reg, rs2: Reg },
    Slt { rd: Reg, rs1: Reg, rs2: Reg },
    Sgt { rd: Reg, rs1: Reg, rs2: Reg },
    Seqz { rd: Reg, rs: Reg },
    Snez { rd: Reg, rs: Reg },
    Xor { rd: Reg, rs1: Reg, rs2: Reg },
    Xori { rd: Reg, rs: Reg, imm12: i32 },
    Or { rd: Reg, rs1: Reg, rs2: Reg },
    Ori { rd: Reg, rs: Reg, imm12: i32 },   
    And { rd: Reg, rs1: Reg, rs2: Reg },
    Andi { rd: Reg, rs: Reg, imm12: i32 },
    Sll { rd: Reg, rs1: Reg, rs2: Reg },
    Srl { rd: Reg, rs1: Reg, rs2: Reg },
    Sra { rd: Reg, rs1: Reg, rs2: Reg },
    Mul { rd: Reg, rs1: Reg, rs2: Reg },
    Div { rd: Reg, rs1: Reg, rs2: Reg },
    Rem { rd: Reg, rs1: Reg, rs2: Reg },
    Li { rd: Reg, imm: i32 },
    La { rd: Reg, label: String },
    Mv { rd: Reg, rs: Reg },
}

pub fn reg2idx(reg: Reg) -> usize {
    match reg {
        "x0" => 0, "ra" => 1, "sp" => 2,
        "gp" => 3, "tp" => 4, "t0" => 5,
        "t1" => 6, "t2" => 7, "fp" => 8,
        "s1" => 9, "a0" => 10, "a1" => 11,
        "a2" => 12, "a3" => 13, "a4" => 14,
        "a5" => 15, "a6" => 16, "a7" => 17,
        "s2" => 18, "s3" => 19, "s4" => 20,
        "s5" => 21, "s6" => 22, "s7" => 23,
        "s8" => 24, "s9" => 25, "s10" => 26,
        "s11" => 27, "t3" => 28, "t4" => 29,
        "t5" => 30, "t6" => 31,
        _ => unreachable!(),
    }
}

const REG_NAME: [Reg; 32] = [
    "x0", "ra", "sp", "gp", "tp", "t0","t1","t2",
    "fp", "s1", "a0", "a1", "a2", "a3", "a4", "a5",
    "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7",
    "s8", "s9", "s10", "s11", "t3", "t4", "t5", "t6",
];


pub fn idx2reg(idx: usize) -> Reg {
    REG_NAME[idx]
}

impl Inst {
    pub fn emit_asm(&self) -> String {
        match self {
            Inst::Beqz { rs, label } => format!("beqz {}, {}", rs, label),
            Inst::Bnez { rs, label } => format!("bnez {}, {}", rs, label),
            Inst::J { label } => format!("j {}", label),
            Inst::Call { label } => format!("call {}", label),
            Inst::Ret => String::from("ret"),
            Inst::Lw { rd, imm12, rs } => format!("lw {}, {}({})", rd, imm12, rs),
            Inst::Sw { rs, imm12, rd } => format!("sw {}, {}({})", rs, imm12, rd),
            Inst::Add { rd, rs1, rs2 } => format!("add {}, {}, {}", rd, rs1, rs2),
            Inst::Addi { rd, rs, imm12 } => format!("addi {}, {}, {}", rd, rs, imm12),
            Inst::Sub { rd, rs1, rs2 } => format!("sub {}, {}, {}", rd, rs1, rs2),
            Inst::Slt { rd, rs1, rs2 } => format!("slt {}, {}, {}", rd, rs1, rs2),
            Inst::Sgt { rd, rs1, rs2 } => format!("sgt {}, {}, {}", rd, rs1, rs2),
            Inst::Seqz { rd, rs } => format!("seqz {}, {}", rd, rs),
            Inst::Snez { rd, rs } => format!("snez {}, {}", rd, rs),
            Inst::Xor { rd, rs1, rs2 } => format!("xor {}, {}, {}", rd, rs1, rs2),
            Inst::Xori { rd, rs, imm12 } => format!("xori {}, {}, {}", rd, rs, imm12),
            Inst::Or { rd, rs1, rs2 } => format!("or {}, {}, {}", rd, rs1, rs2),
            Inst::Ori { rd, rs, imm12 } => format!("ori {}, {}, {}", rd, rs, imm12),
            Inst::And { rd, rs1, rs2 } => format!("and {}, {}, {}", rd, rs1, rs2),
            Inst::Andi { rd, rs, imm12 } => format!("andi {}, {}, {}", rd, rs, imm12),
            Inst::Sll { rd, rs1, rs2 } => format!("sll {}, {}, {}", rd, rs1, rs2),
            Inst::Srl { rd, rs1, rs2 } => format!("srl {}, {}, {}", rd, rs1, rs2),
            Inst::Sra { rd, rs1, rs2 } => format!("sra {}, {}, {}", rd, rs1, rs2),
            Inst::Mul { rd, rs1, rs2 } => format!("mul {}, {}, {}", rd, rs1, rs2),
            Inst::Div { rd, rs1, rs2 } => format!("div {}, {}, {}", rd, rs1, rs2),
            Inst::Rem { rd, rs1, rs2 } => format!("rem {}, {}, {}", rd, rs1, rs2),
            Inst::Li { rd, imm } => format!("li {}, {}", rd, imm),
            Inst::La { rd, label } => format!("la {}, {}", rd, label),
            Inst::Mv { rd, rs } => format!("mv {}, {}", rd, rs),
        }
    }
}