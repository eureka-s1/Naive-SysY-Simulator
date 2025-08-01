// pipeline.rs
// use crate::common::*;
// use crate::memory::*;

use colored::Colorize;

use super::decode::*;
use super::cpu::*;
use super::mem::*;
use super::decode::*;

const MEM_BASE: u64 = 0x8000_0000; 
const MEM_SIZE: usize = 0x80_00000; 

pub struct Pipeline {
    pub cpu: CPUState,
    pub debug_mode: bool,
    
    pub D_reg: IFIDReg,
    pub E_reg: IDEXReg,
    pub M_reg: EXMEMReg,
    pub W_reg: MEMWBReg,
    pub d_reg: IFIDReg,
    pub e_reg: IDEXReg,
    pub m_reg: EXMEMReg,
    pub w_reg: MEMWBReg,
    
    pub f_stall: bool,
    pub d_stall: bool,

    pub branch_count: u32,
    pub data_hazard_count: u32,

}

const NOP_INST: u32 = 0x13; // NOP instruction

impl Pipeline {
    pub fn new() -> Self {
        Self {
            cpu: CPUState::new(),
            debug_mode: false,
            D_reg: IFIDReg::default(),
            E_reg: IDEXReg::default(),
            M_reg: EXMEMReg::default(),
            W_reg: MEMWBReg::default(),
            d_reg: IFIDReg::default(),
            e_reg: IDEXReg::default(),
            m_reg: EXMEMReg::default(),
            w_reg: MEMWBReg::default(),
            f_stall: false,
            d_stall: false,
            branch_count: 0,
            data_hazard_count: 0,
        }
    }

    pub fn init(&mut self) {
        self.cpu.pc = MEM_BASE;
        self.cpu.reg[0] = 0;
        self.cpu.running = true;
        self.cpu.cycle_count = 0;
        self.cpu.inst_count = 0;
        
        self.D_reg.inst = NOP_INST;
        self.E_reg.inst = NOP_INST;
        self.M_reg.inst = NOP_INST;
        self.W_reg.inst = NOP_INST;
        
        self.f_stall = false;
        self.d_stall = false;
    }

    pub fn step(&mut self, mem: &mut Memory) {
        self.cpu.cycle_count += 1;


        self.print_state(mem);

        // Write Back Stage
        writeback_stage(&mut self.cpu, &self.W_reg);

        // Memory Stage
        self.w_reg = memory_stage(&mut self.cpu, &self.M_reg, mem);

        // Execute Stage
        self.m_reg = execute_stage(&mut self.cpu, &self.E_reg);

        // Decode Stage
        self.e_reg = decode_stage(&self.cpu, &self.D_reg);

        // Fetch Stage
        self.d_reg.pc = self.cpu.pc;
        self.d_reg.inst = mem.inst_fetch(self.cpu.pc).expect("Invalid instruction fetch");
        self.cpu.pred_pc = self.cpu.pc.wrapping_add(4);
        
        // // Data hazard detection
        self.data_hazard();
        self.branch_pred_miss();

        // // Update all state 
        self.W_reg = self.w_reg;
        self.M_reg = self.m_reg;
        self.E_reg = self.e_reg;
        if !self.d_stall { self.D_reg = self.d_reg; }
        if !self.f_stall { self.cpu.pc = self.cpu.pred_pc; }

        if self.d_stall { self.d_stall = false; }
        if self.f_stall { self.f_stall = false; }


    }

    fn exec_stall(&mut self) {
        self.f_stall = true;
        self.d_stall = true;

        self.e_reg = IDEXReg {
            inst: NOP_INST,
            ..IDEXReg::default()
        };

        self.data_hazard_count += 1;
    }

    fn data_hazard(&mut self) {
        let alu_a = self.e_reg.rs1; 
        let alu_b = self.e_reg.rs2;

        let dst_e = self.E_reg.rd;
        let dst_m = self.M_reg.rd;

        if (alu_a == dst_e && dst_e != 0 && self.E_reg.store == false) || (alu_b == dst_e && dst_e != 0 && self.E_reg.store == false) || 
        (alu_a == dst_m && dst_m != 0 && self.M_reg.store == false) || (alu_b == dst_m && dst_m != 0 && self.M_reg.store == false) {
        if((alu_a == dst_e && dst_e != 0 && self.E_reg.store == false)){
            // exec_stall();
            if self.E_reg.load == true { self.exec_stall(); }  // load-use hazard
            else { self.e_reg.src1 = self.m_reg.alu_out; }    
        }
        else if ((alu_a == dst_m && dst_m != 0 && self.M_reg.store == false)){
            // exec_stall();
            if self.M_reg.load == true { self.e_reg.src1 = self.w_reg.mem_data; } 
            else { self.e_reg.src1 = self.w_reg.alu_out; }
        }
        
        if (alu_b == dst_e && dst_e != 0 && self.E_reg.store == false) {
            // exec_stall();
            if self.E_reg.load == true { self.exec_stall(); }  // load-use hazard
            else { self.e_reg.src2 = self.m_reg.alu_out; }
        }
        else if (alu_b == dst_m && dst_m != 0 && self.M_reg.store == false) {
            // exec_stall();
            if self.M_reg.load == true { self.e_reg.src2 = self.w_reg.mem_data;  }
            else {self.e_reg.src2 = self.w_reg.alu_out;}
        }
    }
    }

    fn branch_pred_miss(&mut self) {    
        if self.E_reg.jump && self.cpu.next_pc != self.D_reg.pc { /* branch prediction miss */
            self.e_reg = IDEXReg {
                inst: NOP_INST,
                ..IDEXReg::default()
            };
            self.d_reg = IFIDReg {
                inst: NOP_INST,
                ..IFIDReg::default()
            };
            self.d_stall = false; 
            self.f_stall = false;
            self.cpu.pred_pc = self.cpu.next_pc;

            self.branch_count += 1; 
        }
    }

    fn pipe_check_rv64m(&mut self) {

    }

    pub fn print_state(&self, mem: &mut Memory) {
        println!("{}", "CPU State:".green());
        println!("  PC: 0x{:016x}", self.cpu.pc);
        println!("  Cycle: {}, Inst: 0x{:08x}", self.cpu.cycle_count, mem.inst_fetch(self.cpu.pc).expect("Invalid instruction fetch"));
        
        println!("{}", "\nPipeline Registers:".blue());
        println!("  IF/ID: PC=0x{:08x}, INST=0x{:08x}", 
            self.D_reg.pc, self.D_reg.inst);
        println!("  ID/EX: PC=0x{:08x}, INST=0x{:08x}, RD={}, RS1={}, RS2={} src1=0x{:x} src2=0x{:x} imm= 0x{:x}", 
            self.E_reg.pc, self.E_reg.inst, self.E_reg.rd, self.E_reg.rs1, self.E_reg.rs2, self.E_reg.src1, self.E_reg.src2, self.E_reg.imm);
        println!("  EX/MEM: PC=0x{:08x}, INST=0x{:08x}, RD={}, ALU=0x{:016x}", 
            self.M_reg.pc, self.M_reg.inst, self.M_reg.rd, self.M_reg.alu_out);
        println!("  MEM/WB: PC=0x{:08x}, INST=0x{:08x}, RD={}, ALU=0x{:016x}", 
            self.W_reg.pc, self.W_reg.inst, self.W_reg.rd, self.W_reg.alu_out);
        
        println!("\nRegisters:");
        for i in 0..32 {
            if self.cpu.reg[i] != 0 {
                let name = match i {
                    0 => "zero",
                    1 => "ra", 2 => "sp", 3 => "gp", 4 => "tp",
                    5 => "t0", 6 => "t1", 7 => "t2",
                    8 => "s0", 9 => "s1",
                    10 => "a0", 11 => "a1", 12 => "a2", 13 => "a3",
                    14 => "a4", 15 => "a5", 16 => "a6", 17 => "a7",
                    18 => "s2", 19 => "s3", 20 => "s4", 21 => "s5",
                    22 => "s6", 23 => "s7", 24 => "s8", 25 => "s9",
                    26 => "s10", 27 => "s11",
                    28 => "t3", 29 => "t4", 30 => "t5", 31 => "t6",
                    _ => continue,
                };
                println!("  {} (x{}): 0x{:016x}", name, i, self.cpu.reg[i]);
            }
        }
        println!();
    }
}