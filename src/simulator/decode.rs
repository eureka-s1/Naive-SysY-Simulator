use std::arch::x86_64::CpuidResult;

use colored::Colorize;

use super::cpu::*;
use super::mem::*;
use super::instruction::*;

// use bit_field::BitField;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InstType {
    I, U, S, R, J, B, N
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Instruction {
    pub pattern: &'static str,
    pub name: &'static str,
    pub inst_type: InstType,
}


pub fn sext(val: u64, len: usize) -> u64 {
    let sign_bit = 1 << (len - 1);
    if (val & sign_bit) != 0 {
        val | (!0 << len)
    } else {
        val
    }
}

#[macro_export]
macro_rules! bits {
    ($val:expr, $high:expr, $low:expr) => {
        ($val >> $low) & ((1 << ($high - $low + 1)) - 1)
    };
}


#[macro_export]
macro_rules! instpat_match {
    ($inst:expr, $pattern:expr) => {{
        // 移除非空格字符
        let pattern_str = $pattern.replace(" ", "");
        let mut mask = 0u32;
        let mut value = 0u32;
        let mut bit_pos: u32 = 31;
        
        for c in pattern_str.chars() {
            match c {
                '0' => {
                mask |= 1 << bit_pos;
                // value 保持不变（0）
                bit_pos = bit_pos.wrapping_sub(1);
                }
                '1' => {
                mask |= 1 << bit_pos;
                value |= 1 << bit_pos;
                bit_pos = bit_pos.wrapping_sub(1);
                }
                '?' => {
                // 不设置掩码位
                bit_pos = bit_pos.wrapping_sub(1);
                }
                _ => panic!("Invalid character in pattern: {}", c),
            }
            
            if bit_pos > 31 {
                break; // 防止下溢
            }
        }
        
        ($inst & mask) == value
    }};
}

pub fn inst_match(inst: u32, pattern: &'static str) -> bool {
    instpat_match!(inst, pattern)
}

pub fn check_inst(inst: u32) -> Option<&'static Instruction> {
    for pattern in INSTRUCTIONS {
        if inst_match(inst, pattern.pattern) {
            return Some(pattern);
        }
    }
    None
}

pub fn decode_stage(cpu: &CPUState, s: &IFIDReg) -> IDEXReg {

    let inst = s.inst;
    let match_res = check_inst(inst);

    let (name, ty) = match match_res {
        None => {
            println!("{}", "Error".red());
            panic!("Invalid instruction: 0x{:x}", inst);
        },
        Some(_inst) => (_inst.name, _inst.inst_type),
    };

    let rd = bits!(inst, 11, 7) as i32;
    let rs1 = bits!(inst, 19, 15) as i32;
    let rs2 = bits!(inst, 24, 20) as i32;
                
    let src1 = if matches!(ty, InstType::I | InstType::S | InstType::B | InstType::R) {
        cpu.reg[rs1 as usize]
    } else { 0 };
    
    let src2 = if matches!(ty, InstType::S | InstType::B | InstType::R) {
        cpu.reg[rs2 as usize]
    } else { 0 };

    let imm = match ty {
        InstType::I => sext(bits!(inst, 31, 20) as u64, 12),
        InstType::U => sext(bits!(inst, 31, 12) as u64, 20) << 12,
        InstType::J => {
            let imm_raw = (bits!(inst, 31, 31) << 20)
                | (bits!(inst, 19, 12) << 12)
                | (bits!(inst, 20, 20) << 11)
                | (bits!(inst, 30, 21) << 1);
            sext(imm_raw as u64, 21)
        }
        InstType::S => {
            let imm_raw = (bits!(inst, 31, 25) << 5) | bits!(inst, 11, 7);
            sext(imm_raw as u64, 12)
        }
        InstType::B => {
            let imm_raw = (bits!(inst, 31, 31) << 12)
                | (bits!(inst, 7, 7) << 11)
                | (bits!(inst, 30, 25) << 5)
                | (bits!(inst, 11, 8) << 1);
            sext(imm_raw as u64, 13)
        }
        _ => 0,
    };

    let jump = matches!(name, "jal" | "jalr" | "beq" | "bne" | "blt" | "bge" | "bltu" | "bgeu");
    let load = matches!(name, "lb" | "lh" | "lw" | "ld" | "lbu" | "lhu");
    let store = matches!(name, "sb" | "sh" | "sw" | "sd");

    IDEXReg {
        pc: s.pc,
        inst: s.inst,
        rd, rs1, rs2,
        src1, src2, imm,
        jump, load, store,
    }
}


pub fn execute_stage(cpu: &mut CPUState, s: &IDEXReg) -> EXMEMReg {
    let inst = s.inst;
    let match_res = check_inst(inst);

    let (name, ty) = match match_res {
        None => {
            println!("{}", "Error".red());
            panic!("Invalid instruction: 0x{:x}", inst);
        },
        Some(_inst) => (_inst.name, _inst.inst_type),
    };

    let src1 = s.src1;
    let src2 = s.src2;
    let imm = s.imm;
    let mut alu_out = 0;

    println!("pc:= {} imm:={}", s.pc, imm);

    match name {
        "lui"    => alu_out = imm,
        "auipc"  => alu_out = s.pc + imm,
        "jal"    => { cpu.next_pc = s.pc + imm; alu_out = s.pc + 4; },
        "jalr"   => { cpu.next_pc = (src1 + imm) & !1; alu_out = s.pc + 4; },
        "beq"    => cpu.next_pc = if src1 == src2 { s.pc + imm } else { s.pc + 4 },
        "bne"    => cpu.next_pc = if src1 != src2 { s.pc + imm } else { s.pc + 4 },
        "blt"    => cpu.next_pc = if (src1 as i64) < (src2 as i64) { s.pc + imm } else { s.pc + 4 },
        "bge"    => cpu.next_pc = if (src1 as i64) >= (src2 as i64) { s.pc + imm } else { s.pc + 4 },
        "bltu"   => cpu.next_pc = if src1 < src2 { s.pc + imm } else { s.pc + 4 },
        "bgeu"   => cpu.next_pc = if src1 >= src2 { s.pc + imm } else { s.pc + 4 },
        "lb" | "lh" | "lw" | "lbu" | "lhu" | "lwu" | "ld" => alu_out = src1 + imm,
        "sb" | "sh" | "sw" | "sd" => alu_out = src1 + imm,
        "addi"   => alu_out = src1.wrapping_add(imm),
        "slti"   => alu_out = if (src1 as i64) < (imm as i64) { 1 } else { 0 },
        "sltiu"  => alu_out = if src1 < imm { 1 } else { 0 },
        "xori"   => alu_out = src1 ^ imm,
        "ori"    => alu_out = src1 | imm,
        "andi"   => alu_out = src1 & imm,
        "slli"   => alu_out = src1 << imm,
        "srli"   => alu_out = src1 >> imm,
        "srai"   => alu_out = ((src1 as i64) >> (imm & 0x3F)) as u64,
        "addiw"  => alu_out = (src1.wrapping_add(imm) as i32) as u64,
        "slliw"  => alu_out = (src1.wrapping_shl(imm as u32) as i32) as u64,
        "srliw"  => alu_out = ((src1 as u32) >> (imm & 0x1F)) as u64,
        "sraiw"  => alu_out = ((src1 as i32) >> (imm & 0x1F)) as u64,
        "add"    => alu_out = src1 + src2,
        "sub"    => alu_out = src1 - src2,
        "sll"    => alu_out = src1 << (src2 & 0x3F),
        "slt"    => alu_out = if (src1 as i64) < (src2 as i64) { 1 } else { 0 },
        "sltu"   => alu_out = if src1 < src2 { 1 } else { 0 },
        "xor"    => alu_out = src1 ^ src2,
        "srl"    => alu_out = src1 >> (src2 & 0x3F),
        "sra"    => alu_out = ((src1 as i64) >> (src2 & 0x3F)) as u64,
        "or"     => alu_out = src1 | src2,
        "and"    => alu_out = src1 & src2,
        "addw"   => alu_out = (src1.wrapping_add(src2) as i32) as u64,
        "subw"   => alu_out = (src1.wrapping_sub(src2) as i32) as u64,
        "sllw"   => alu_out = (src1.wrapping_shl(src2 as u32 & 0x1F) as i32) as u64,
        "srlw"   => alu_out = ((src1 as u32) >> (src2 & 0x1F)) as u64,
        "sraw"   => alu_out = ((src1 as i32) >> (src2 & 0x1F)) as u64,
        "ebreak" => cpu.halt_trap(s.pc, cpu.reg[10]), // a0 
        "mul"    => alu_out = (src1 as i64).wrapping_mul(src2 as i64) as u64,
        "mulh"   => alu_out = ((src1 as i128) * (src2 as i128) >> 64) as u64,
        "mulhsu" => alu_out = (((src1 as i128) * (src2 as u128) as i128) >> 64) as u64,
        "mulhu"  => alu_out = ((src1 as u128) * (src2 as u128) >> 64) as u64,
        "div"    => alu_out = (src1 as i64).wrapping_div(src2 as i64) as u64,
        "divu"   => alu_out = src1.wrapping_div(src2),
        "rem"    => alu_out = (src1 as i64).wrapping_rem(src2 as i64) as u64,
        "remu"   => alu_out = src1.wrapping_rem(src2),
        "mulw"   => alu_out = (src1 as i32).wrapping_mul(src2 as i32) as u64,
        "divw"   => alu_out = (src1 as i32).wrapping_div(src2 as i32) as u64,
        "divuw"  => alu_out = (src1 as u32).wrapping_div(src2 as u32) as u64,
        "remw"   => alu_out = (src1 as i32).wrapping_rem(src2 as i32) as u64,
        "remuw"  => alu_out = (src1 as u32).wrapping_rem(src2 as u32) as u64,
        _ => {},
    }

    EXMEMReg { 
        pc: s.pc,
        inst: s.inst,
        rd: s.rd,
        src2: s.src2,
        alu_out: alu_out,  
        store: s.store,
        load: s.load,
    }

}

pub fn memory_stage(cpu: &mut CPUState, s: &EXMEMReg, mem: &mut Memory) -> MEMWBReg {
    let inst = s.inst;
    let match_res = check_inst(inst);

    let (name, ty) = match match_res {
        None => {
            println!("{}", "Error".red());
            panic!("Invalid instruction: 0x{:x}", inst);
        },
        Some(_inst) => (_inst.name, _inst.inst_type),
    };

    let alu_out = s.alu_out;
    let src2 = s.src2;
    let mut mem_data = 0;

    match name {
        "lb" => mem_data = sext(mem.mem_read(alu_out, 1).unwrap(), 8),
        "lh" => mem_data = sext(mem.mem_read(alu_out, 2).unwrap(), 16),
        "lw" => mem_data = sext(mem.mem_read(alu_out, 4).unwrap(), 32),
        "lbu" => mem_data = sext(mem.mem_read(alu_out, 1).unwrap(), 8),
        "lhu" => mem_data = sext(mem.mem_read(alu_out, 2).unwrap(), 16),
        "lwu" => mem_data = sext(mem.mem_read(alu_out, 4).unwrap(), 32),
        "ld" => mem_data = sext(mem.mem_read(alu_out, 8).unwrap(), 64),
        "sb" => mem.mem_write(alu_out, 1, src2).unwrap(),
        "sh" => mem.mem_write(alu_out, 2, src2).unwrap(),
        "sw" => mem.mem_write(alu_out, 4, src2).unwrap(),
        "sd" => mem.mem_write(alu_out, 8, src2).unwrap(),
        "ebreak" => cpu.halt_trap(s.pc, cpu.reg[10]),
        _ => (),
    }

    MEMWBReg {
        pc: s.pc,
        inst: s.inst,
        rd: s.rd,
        alu_out: alu_out,
        mem_data: mem_data,
        load: s.load,
        store: s.store,
    }

}

pub fn writeback_stage(cpu: &mut CPUState, s: &MEMWBReg) {
    let inst = s.inst;
    let match_res = check_inst(inst);

    let (name, ty) = match match_res {
        None => {
            println!("{}", "Error".red());
            panic!("Invalid instruction: 0x{:x}", inst);
        },
        Some(_inst) => (_inst.name, _inst.inst_type),
    };

    let alu_out = s.alu_out;
    let mem_data = s.mem_data;
    let rd = s.rd;

    match name {
        "beq" | "bne" | "blt" | "bge" | "bltu" | "bgeu" => (),
        "sb" | "sh" | "sw" | "sd" => (),
        "lb" | "lh" | "lw" | "ld" | "lbu" | "lhu" => cpu.reg[rd as usize] = mem_data,
        "ebreak" => cpu.halt_trap(s.pc, cpu.reg[10]),
        _ => cpu.reg[rd as usize] = alu_out,
    }
    cpu.reg[0] = 0;

}