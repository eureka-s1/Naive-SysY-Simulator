use colored::Colorize;

const MEM_BASE: u64 = 0x8000_0000; 
const MEM_SIZE: usize = 0x80_00000; 


#[derive(Debug)]
pub struct CPUState {
    pub reg: [u64; 32],
    pub pc: u64,
    pub running: bool,

    /* Sequential execution state */
    pub next_pc: u64,
    
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
}


impl CPUState {
    pub fn new() -> Self {
        Self {
            reg: [0; 32],
            pc: MEM_BASE,
            running: false,
            next_pc: 0,
            pred_pc: 0,
            cycle_count: 0,
            inst_count: 0,
            branch_count: 0,
            data_hazard_count: 0,
        }
    }

    
    pub fn halt_trap(&mut self, pc: u64 , code: u64){
        if code != 0 {
            println!("{}", "HIT BAD TRAP!".red());
        }else{
            println!("{}", "HIT GOOD TRAP!".green());
            println!("Total instructions executed: {}", self.inst_count);
            println!("Total cycles: {}\n", self.cycle_count);
            println!("Total Data Hazard: {}\n", self.data_hazard_count);
            println!("Total Branch Misprediction: {}", self.branch_count);   
        }
        println!("Program ended at pc 0x{:08x}, with exit code {}", pc, code);
        self.running = false;
    }
}

