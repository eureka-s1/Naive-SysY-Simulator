const MEM_BASE: u64 = 0x8000_0000; 
const MEM_SIZE: usize = 0x80_00000; 


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Fetch,
    Decode,
    Execute,
    Memory,
    Writeback,
}

#[derive(Debug)]
pub struct CPUState {
    pub reg: [u64; 32],
    pub pc: u64,
    
    /* Sequential execution state */
    pub stage: Stage,
    pub next_pc: u64,
    pub inst: u32,
    pub src1: u64,
    pub src2: u64,
    pub imm: u64,
    pub rd: i32,
    pub alu_out: u64,
    pub mem_data: u64,
    
    /* Pipeline state */
    pub pred_pc: u64,
    
    /* Performance counters */
    pub cycle_count: i32,
    pub inst_count: i32,
    pub branch_count: i32,
    pub data_hazard_count: i32,
}

/* Pipeline registers */
#[derive(Debug, Default, Clone, Copy)]
pub struct IFIDReg {
    pub pc: u64,
    pub inst: u32,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct IDEXReg {
    pub pc: u64,
    pub inst: u32,
    
    pub rd: i32,
    pub rs1: i32,
    pub rs2: i32,
    
    pub src1: u64,
    pub src2: u64,
    pub imm: u64,

    pub jump: bool,
    pub load: bool,
    pub store: bool,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct EXMEMReg {
    pub pc: u64,
    pub inst: u32,
    pub rd: i32,
    pub src2: u64,
    pub alu_out: u64,

    
    pub load: bool,
    pub store: bool,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MEMWBReg {
    pub pc: u64,
    pub inst: u32,
    pub rd: i32,
    pub alu_out: u64,
    pub mem_data: u64,

    pub load: bool,
    pub store: bool,
}

pub enum RegStage {
    IFIDReg,
    IDEXReg,
    EXMEMReg,
    MEMWBReg,
}

// Stage transition
trait Decode {
    fn decode(&mut self, inst: u32) -> RegStage;
}

impl CPUState {
    pub fn new() -> Self {
        Self {
            reg: [0; 32],
            pc: MEM_BASE,
            stage: Stage::Fetch,
            next_pc: 0,
            inst: 0,
            src1: 0,
            src2: 0,
            imm: 0,
            rd: 0,
            alu_out: 0,
            mem_data: 0,
            pred_pc: 0,
            cycle_count: 0,
            inst_count: 0,
            branch_count: 0,
            data_hazard_count: 0,
        }
    }
}