use super::instruction::*;
use super::array::InitVal;

use super::instruction::Inst;
use super::label::Label;

#[derive(Debug, Clone)]
pub struct AsmProgram {
    global_defs: Vec<AsmGlobalDef>,
    globals: Vec<AsmGlobal>,
}

#[derive(Debug, Clone)]
pub struct AsmGlobalDef {
    pub label: Label,
    pub init_val: Vec<InitVal>,
}

#[derive(Debug, Clone)]
pub struct AsmGlobal {
    section: Section,
    label: Label,
    frame_size: i32,
    locals: Vec<AsmLocal>,
}

#[derive(Debug, Clone)]
pub enum Section {
    Text,
    Data,
}

// .data section does not have label
#[derive(Debug, Clone)]
pub struct AsmLocal {
    label: Option<Label>,
    insts: Vec<Inst>,
}

impl AsmProgram {
    pub fn new() -> Self {
        Self {
            global_defs: Vec::new(),
            globals: Vec::new(),
        }
    }

    pub fn push_global(&mut self, global: AsmGlobal) {
        self.globals.push(global);
    }

    pub fn push_globaldef(&mut self, globaldef: AsmGlobalDef) {
        self.global_defs.push(globaldef);
    }
    
    pub fn emit_asm(&self) -> String {
        let mut asm_txt = String::new();

        
        asm_txt.push_str(".globl _trm_init\n");
        asm_txt.push_str(".globl _start\n");

        for global_def in &self.global_defs {
            asm_txt.push_str(&format!(".globl {}\n", global_def.label.to_string()));
        }
        for global in &self.globals {
            asm_txt.push_str(&format!(".globl {}\n", global.label.name()));
        };

        
        asm_txt.push_str(".section .data\n");
        for global_def in &self.global_defs {
            asm_txt.push_str(&global_def.emit_asm());
            asm_txt.push_str(&format!("\n"));
        }


        

        asm_txt.push_str(".section .text\n");
        asm_txt.push_str("_start:
  la sp, stack_top        
  jal _trm_init\n");

        asm_txt.push_str("_trm_init:
  addi sp, sp, -16
  sd ra, 8(sp)
  jal main
  ebreak\n");
        

        for global in &self.globals {
            asm_txt.push_str(&global.emit_asm());
            asm_txt.push_str(&format!("\n"));
        };

        asm_txt.push_str(".section .bss
.align 4
stack_bottom:
  .skip 4096
stack_top:\n");

        asm_txt
    }
}


impl AsmGlobalDef {
    pub fn emit_asm(&self) -> String {
        let mut asm_txt = String::new();
        // asm_txt.push_str(&format!("  .globl {}\n", self.label.to_string()));
        asm_txt.push_str(&format!("{}:\n", self.label.to_string()));
        for init_val in &self.init_val {
            match init_val {
                InitVal::Word(val) => {
                    asm_txt.push_str(&format!("  .word {}\n", val));
                },
                InitVal::Zero(size) => {
                    asm_txt.push_str(&format!("  .zero {}\n", size.clone() as i32));
                },
                InitVal::Array(vec) => {
                    for val in vec {
                        match val {
                            InitVal::Word(val) => {
                                asm_txt.push_str(&format!("  .word {}\n", val));
                            },
                            InitVal::Zero(size) => {
                                asm_txt.push_str(&format!("  .zero {}\n", size.clone() as i32));
                            },
                            _ => panic!("Unsupport"),
                        }
                    }
                }
            }
        }
        asm_txt
    }
}

impl AsmGlobal {
    pub fn new(section: Section, label: Label) -> Self {
        Self {
            section,
            label,
            frame_size: 0 as i32,
            locals: Vec::new(),
        }
    }

    pub fn push_local(&mut self, local: AsmLocal) {
        self.locals.push(local);
    }

    pub fn emit_asm(&self) -> String {
        let mut asm_txt = String::new();
        // match self.section {
        //     Section::Text => {
        //         asm_txt.push_str(&format!("  .text\n"));
        //         // asm_txt.push_str(&format!("  .globl {}\n", self.label.to_string()));
        //         asm_txt.push_str(&format!("  .globl {}\n{}:\n", self.label.name(), self.label.name()));
        //     },
        //     Section::Data => {
        //         asm_txt.push_str(&format!("  .data\n"));
        //     },
        // };
        asm_txt.push_str(&format!("{}:\n", self.label.name()));

        for (index, local) in self.locals.iter().enumerate() {
            // skip the first label
            if index > 0 {
                if let Some(label) = &local.label {
                    asm_txt.push_str(&format!("{}:\n", label.name()));
                }
            }
            asm_txt.push_str(&local.emit_asm());
        }
        asm_txt
    }
}

pub fn is_imm12(imm: i32) -> bool {
    imm >= -2048 && imm <= 2047
}

impl AsmLocal {
    pub fn new(label: Option<Label>) -> Self {
        Self {
            label,
            insts: Vec::new(),
        }
    }

    pub fn push_inst(&mut self, inst: Inst) {
        self.insts.push(inst);
    }

    pub fn beqz_inst(&mut self, rs: Reg, label: String) {
        self.push_inst(Inst::Beqz { rs: rs, label: label });
    }

    pub fn bnez_inst(&mut self, rs: Reg, label: String) {
        self.push_inst(Inst::Bnez { rs: rs, label: label });
    }

    pub fn J_inst(&mut self, label: String) {
        self.push_inst(Inst::J { label: label });
    }

    pub fn call_inst(&mut self, label: String) {
        self.push_inst(Inst::Call { label: label });
    }

    pub fn ret_inst(&mut self) {
        self.push_inst(Inst::Ret);
    }

    pub fn lw_inst(&mut self, rd: Reg, imm: i32, rs: Reg) {
        // check imm12
        if is_imm12(imm) {
            self.push_inst(Inst::Lw {  rd: rd, imm12: imm, rs: rs });
        } else {
            let temp = "t0";
            self.push_inst(Inst::Li  { rd: temp, imm: imm});
            self.push_inst(Inst::Add { rd: temp, rs1: rs , rs2: temp });
            self.push_inst(Inst::Lw  { rd: rd  , imm12: 0, rs: temp });
        }
    }

    pub fn sw_inst(&mut self, rs: Reg, imm: i32, rd: Reg) {
        // check imm12
        if is_imm12(imm) {
            self.push_inst(Inst::Sw { rs: rs, imm12: imm, rd: rd });
        } else {
            let temp = "t0";
            self.push_inst(Inst::Li  { rd: temp, imm  : imm });
            self.push_inst(Inst::Add { rd: temp, rs1  : rd , rs2: temp });
            self.push_inst(Inst::Sw  { rs: rs  , imm12: 0  , rd : temp });
        }
    }

    pub fn add_inst(&mut self, rd: Reg, rs1: Reg, rs2: Reg) {
        self.push_inst(Inst::Add { rd: rd, rs1: rs1, rs2: rs2 });
    }

    pub fn addi_inst(&mut self, rd: Reg, rs: Reg, imm: i32) {
        if is_imm12(imm) {
            self.push_inst(Inst::Addi { rd: rd, rs: rs, imm12: imm });
        }
        else {
            let temp = "t0";
            self.push_inst(Inst::Li { rd: temp, imm: imm });
            self.push_inst(Inst::Add { rd: rd, rs1: rs, rs2: temp });
        }
        
    }

    pub fn sub_inst(&mut self, rd: Reg, rs1: Reg, rs2: Reg) {
        self.push_inst(Inst::Sub { rd: rd, rs1: rs1, rs2: rs2 });
    }

    pub fn slt_inst(&mut self, rd: Reg, rs1: Reg, rs2: Reg) {
        self.push_inst(Inst::Slt { rd: rd, rs1: rs1, rs2: rs2 });
    }

    pub fn sgt_inst(&mut self, rd: Reg, rs1: Reg, rs2: Reg) {
        self.push_inst(Inst::Sgt { rd: rd, rs1: rs1, rs2: rs2 });
    }

    pub fn seqz_inst(&mut self, rd: Reg, rs: Reg) {
        self.push_inst(Inst::Seqz { rd: rd, rs: rs });
    }

    pub fn snez_inst(&mut self, rd: Reg, rs: Reg) {
        self.push_inst(Inst::Snez { rd: rd, rs: rs });
    }

    pub fn xor_inst(&mut self, rd: Reg, rs1: Reg, rs2: Reg) {
        self.push_inst(Inst::Xor { rd: rd, rs1: rs1, rs2: rs2 });
    }

    pub fn xori_inst(&mut self, rd: Reg, rs: Reg, imm: i32) {
        self.push_inst(Inst::Xori { rd: rd, rs: rs, imm12: imm });
    }

    pub fn or_inst(&mut self, rd: Reg, rs1: Reg, rs2: Reg) {
        self.push_inst(Inst::Or { rd: rd, rs1: rs1, rs2: rs2 });
    }

    pub fn ori_inst(&mut self, rd: Reg, rs: Reg, imm: i32) {
        self.push_inst(Inst::Ori { rd: rd, rs: rs, imm12: imm });
    }

    pub fn and_inst(&mut self, rd: Reg, rs1: Reg, rs2: Reg) {
        self.push_inst(Inst::And { rd: rd, rs1: rs1, rs2: rs2 });
    }

    pub fn andi_inst(&mut self, rd: Reg, rs: Reg, imm: i32) {
        self.push_inst(Inst::Andi { rd: rd, rs: rs, imm12: imm });
    }

    pub fn sll_inst(&mut self, rd: Reg, rs1: Reg, rs2: Reg) {
        self.push_inst(Inst::Sll { rd: rd, rs1: rs1, rs2: rs2 });
    }

    pub fn srl_inst(&mut self, rd: Reg, rs1: Reg, rs2: Reg) {
        self.push_inst(Inst::Srl { rd: rd, rs1: rs1, rs2: rs2 });
    }

    pub fn sra_inst(&mut self, rd: Reg, rs1: Reg, rs2: Reg) {
        self.push_inst(Inst::Sra { rd: rd, rs1: rs1, rs2: rs2 });
    }

    pub fn mul_inst(&mut self, rd: Reg, rs1: Reg, rs2: Reg) {
        self.push_inst(Inst::Mul { rd: rd, rs1: rs1, rs2: rs2 });
    }

    pub fn div_inst(&mut self, rd: Reg, rs1: Reg, rs2: Reg) {
        self.push_inst(Inst::Div { rd: rd, rs1: rs1, rs2: rs2 });
    }

    pub fn rem_inst(&mut self, rd: Reg, rs1: Reg, rs2: Reg) {
        self.push_inst(Inst::Rem { rd: rd, rs1: rs1, rs2: rs2 });
    }

    pub fn li_inst(&mut self, rd: Reg, imm: i32) {
        self.push_inst(Inst::Li { rd: rd, imm: imm });
    }

    pub fn la_inst(&mut self, rd: Reg, label: String) {
        self.push_inst(Inst::La { rd: rd, label: label });
    }

    pub fn mv_inst(&mut self, rd: Reg, rs: Reg) {
        self.push_inst(Inst::Mv { rd: rd, rs: rs });
    }

    // actually use mul
    pub fn muli_inst(&mut self, rd: Reg, rs: Reg, imm: i32) {
        let temp = "t0";
        self.push_inst(Inst::Li { rd: temp, imm: imm });
        self.push_inst(Inst::Mul { rd: rd, rs1: rs, rs2: temp });
    }


    pub fn emit_asm(&self) -> String {
        let mut asm_txt = String::new();
        for inst in &self.insts {
            asm_txt.push_str(&format!("  {}\n", inst.emit_asm()));
        }
        asm_txt
    }
}