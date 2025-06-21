
use super::instruction::{Reg, reg2idx, idx2reg};

use std::collections::HashMap;
use koopa::ir::{Value, ValueKind};


enum Descriptor {
    
}

pub struct DescriptorTable {
    pub reg2val: [Option<Value>; 32],
    pub val2reg: HashMap<Value, Reg>,
}

impl DescriptorTable {
    pub fn new() -> Self {
        Self {
            reg2val: [None; 32],
            val2reg: HashMap::new(),
        }
    }
    
    pub fn get_reg(&self) -> usize {
        for i in 0..32 {
            if self.reg2val[i] == None {
                if (i==9) || (i==6) || (i==7) || (i >= 18) {
                    return i;
                }
            }
        }
        panic!("All register is used")
    }

    pub fn free_reg(&mut self, val: Value, reg: Reg) {
        if reg == "x0" { 
            self.val2reg.remove(&val);
            return;
        }
        let idx = reg2idx(reg);
        println!("free reg {}",idx2reg(idx));
        let val1 = self.reg2val[idx].expect("Double free");
        assert_eq!(val, val1, "Free on unmatched pair");
        self.reg2val[idx] = None;
        self.val2reg.remove(&val);
    }

    pub fn alloc_reg(&mut self, val: Value, rd: Option<Reg>) -> Reg {
        if let Some(reg) = rd {
            match reg {
                "x0" => (),
                _ => self.reg2val[reg2idx(reg)] = Some(val),
            };
            self.val2reg.insert(val, reg);
            return reg;
        } else {
            let idx = self.get_reg();
            self.reg2val[idx] = Some(val);
            self.val2reg.insert(val, idx2reg(idx));
            return idx2reg(idx);
        }
    }

    pub fn reg_move_to(&mut self, rs: Reg, rd: Reg) -> bool {
        if rs == rd {
            return false;
        }
        let rs_idx = reg2idx(rs);
        let rd_idx = reg2idx(rd);
        let val = self.reg2val[rs_idx].expect("rs is free!");
        match self.reg2val[rd_idx] {
            None => {
                self.reg2val[rd_idx] = Some(val);
                self.reg2val[rs_idx] = None;
                self.val2reg.insert(val, rd);
                false
            }
            Some(_val) => {
                self.reg2val[rs_idx] = Some(_val);
                self.reg2val[rd_idx] = Some(val);
                self.val2reg.insert(val, rd);
                self.val2reg.insert(_val, rs);
                true
            }
        }
    }
}
