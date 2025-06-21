use std::char::ParseCharError;
use std::collections::HashMap;

use crate::codegen::asm::AsmGlobalDef;

use super::asm::{AsmProgram, AsmGlobal, AsmLocal, Section};
use super::instruction::Inst;
use super::label::Label;
use super::instruction::Reg;
use super::table::DescriptorTable;
use super::instruction::{reg2idx, idx2reg};
use super::array::InitVal;
use super::generate::PARA_REG;

use koopa::front::ast::Aggregate;
use koopa::ir::entities::ValueData;
use koopa::ir::*;
use rand::distributions::{Alphanumeric, Uniform};
use rand::prelude::Distribution;
use rand::Rng;


pub struct Context<'a> {
    pub program: &'a Program,
    pub function: Option<Function>,
    pub block: Option<BasicBlock>,
}

impl<'a> Context<'a> {
    pub fn new(program: &'a Program) -> Self {
        Self {
            program,
            function: None,
            block: None,
        }
    }
}

// environment for code generation
pub struct Env<'a> {
    pub ctx: Context<'a>,
    pub frame_size: i32,
    pub table: DescriptorTable,
    pub offset: HashMap<Value, usize>,
    pub paranum : usize,
    pub global_var: HashMap<Value, String>,
}

impl<'a> Env<'a> {
    pub fn new(program: &'a Program) -> Self {
        Self { 
            ctx: Context::new(program), 
            frame_size: 128,
            table: DescriptorTable::new(),
            offset: HashMap::new(),
            paranum: 0,
            global_var: HashMap::new(),
        }
    }

    pub fn find_global_name(&mut self, val: Value) -> String {
        
        println!("Search: global val = {:?}", val);
        println!("Result: {:#?}", self.global_var.get(&val));
        match self.global_var.get(&val) {
            None => {
                let name = generate_random_string(7);
                self.global_var.insert(val, name.clone());
                name
            }
            Some(name) => name.clone(),
        } 
    }

    pub fn print_state(&self) {
        for val in self.table.reg2val.iter().enumerate() {
            if let Some(reg) = val.1 {
                println!("{} {:?}", idx2reg(val.0), reg);
            }
        }
    }

    pub fn move_inst(&mut self, asm_bb: &mut AsmLocal, val: Value, rd: Option<Reg>) -> Reg {
        let func_data = self.ctx.program.func(self.ctx.function.unwrap());
        let value_data = func_data.dfg().value(val);
        match value_data.kind() {
            ValueKind::Integer(int) => self.build_int(asm_bb, val, int.clone(), rd).unwrap(),
            ValueKind::FuncArgRef(funargref) => self.build_func_arg_ref(asm_bb, val, funargref.clone()).unwrap(),
            _ => self.move_inst_to(asm_bb, val, rd),
        }
    }

    fn move_inst_to(&mut self, asm_bb: &mut AsmLocal, inst: Value, dst: Option<Reg>) -> Reg {
        let rs = *self.table.val2reg.get(&inst).expect("No reg has value inst");
        if dst == None || dst == Some(rs) {
            return rs;
        } else {
            let rd = dst.unwrap();
            let need_swap = self.table.reg_move_to(rs, rd);
            if need_swap {
                let temp = "t0";
                asm_bb.mv_inst(temp, rd);
                asm_bb.mv_inst(rd, rs );
                asm_bb.mv_inst(rs,temp );
            } else {
                asm_bb.mv_inst(rd, rs);
            }
            return rd;
        }
    }

    pub fn build_inst(&mut self, asm_bb: &mut AsmLocal, val: Value, rd: Option<Reg>, kind: &ValueKind) 
        -> Option<Reg> {
        let res = match kind {
            ValueKind::Integer(int) => self.build_int(asm_bb,val, int.clone(), rd),
            ValueKind::Return(ret) => self.build_ret(asm_bb, val, ret, rd),
            ValueKind::Binary(bin) => self.build_binary(asm_bb, val, bin, rd),
            ValueKind::Branch(branch) => self.build_branch(asm_bb, val, branch, rd),
            ValueKind::Jump(jump) => self.build_jump(asm_bb, val, jump, rd),
            ValueKind::Alloc(_) => None,
            ValueKind::Store(store) => self.build_store(asm_bb, val, store, rd),
            ValueKind::Load(load) => self.build_load(asm_bb, val, load, rd),
            ValueKind::Call(call) => self.build_call(asm_bb, val, call, rd),
            ValueKind::GetElemPtr(getelemptr) => self.build_getelem_ptr(asm_bb, val, getelemptr, rd),
            ValueKind::GetPtr(getptr) => self.build_get_ptr(asm_bb, val, getptr, rd),
            _ => panic!("Unsupported value kind"),
        };


        let func_data = self.ctx.program.func(self.ctx.function.unwrap());
        let value_data = func_data.dfg().value(val);
        let used_by = value_data.used_by();
        if used_by.len() == 1 {
            let &callee = used_by.iter().next().unwrap();
            if let ValueKind::Call(..) = func_data.dfg().value(callee).kind() {
                let reg = res.unwrap();
                let imm = self.offset.get(&val).unwrap().clone() as i32;
                asm_bb.sw_inst(reg, imm, "sp");
                self.table.free_reg(val, reg);
            }
        }
        res 
    }

    pub fn build_int(&mut self, asm_bb: &mut AsmLocal, val:Value, int: values::Integer, rd: Option<Reg>) 
        -> Option<Reg> {
        let rd = self.table.alloc_reg(val, rd);
        asm_bb.li_inst(rd, int.value());
        Some(rd)
    }

    pub fn build_ret(&mut self, asm_bb: &mut AsmLocal, val: Value, ret: &values::Return, rd: Option<Reg>) -> Option<Reg> {
        if let Some(ret_val) = ret.value() {
            self.move_inst(asm_bb, ret_val, Some("a0"));
        }
        asm_bb.lw_inst("ra", self.frame_size - 4, "sp");
        asm_bb.addi_inst("sp", "sp", self.frame_size);
        if let Some(ret_val) = ret.value() {
            self.table.free_reg(ret_val, "a0");
        }
        asm_bb.ret_inst();
        None
    }

    pub fn build_func_arg_ref(&mut self, asm_bb: &mut AsmLocal, val: Value, funargref: values::FuncArgRef) -> Option<Reg> {
        let idx = funargref.index() as usize;
        if idx < 8 {
            Some(PARA_REG[idx])
        } else {
            let imm = (idx - 8) * 4 + self.frame_size as usize;
            let dst = self.table.alloc_reg(val, None);
            asm_bb.lw_inst(dst, imm as i32, "sp");
            Some(dst)
        }
    }

    pub fn build_binary(&mut self, asm_bb: &mut AsmLocal, val: Value, bin: &values::Binary, rd: Option<Reg>) -> Option<Reg> {
        let rd = self.table.alloc_reg(val, rd);
        let rs1 = self.move_inst(asm_bb, bin.lhs(), None);
        let rs2 = self.move_inst(asm_bb, bin.rhs(), None);
        match bin.op() {
            BinaryOp::Add => asm_bb.add_inst(rd, rs1, rs2),
            BinaryOp::Sub => asm_bb.sub_inst(rd, rs1, rs2),
            BinaryOp::Mul => asm_bb.mul_inst(rd, rs1, rs2),
            BinaryOp::Div => asm_bb.div_inst(rd, rs1, rs2),
            BinaryOp::Mod => asm_bb.rem_inst(rd, rs1, rs2),
            BinaryOp::Eq => {
                asm_bb.xor_inst(rd, rs1, rs2);
                asm_bb.seqz_inst(rd, rd);
            }
            BinaryOp::NotEq => {
                asm_bb.xor_inst(rd, rs1, rs2);
                asm_bb.snez_inst(rd, rd);
            }
            BinaryOp::Gt => asm_bb.sgt_inst(rd, rs1, rs2),
            BinaryOp::Lt => asm_bb.slt_inst(rd, rs1, rs2),
            BinaryOp::Le => {
                asm_bb.sgt_inst(rd, rs1, rs2);
                asm_bb.seqz_inst(rd, rd);
            }
            BinaryOp::Ge => {
                asm_bb.slt_inst(rd, rs1, rs2);
                asm_bb.seqz_inst(rd, rd);
            }
            BinaryOp::And => asm_bb.and_inst(rd, rs1, rs2),
            BinaryOp::Or => asm_bb.or_inst(rd, rs1, rs2),
            BinaryOp::Xor => asm_bb.xor_inst(rd, rs1, rs2),
            BinaryOp::Sar => asm_bb.sra_inst(rd, rs1, rs2),
            BinaryOp::Shl => asm_bb.sll_inst(rd, rs1, rs2),
            BinaryOp::Shr => asm_bb.srl_inst(rd, rs1, rs2),
        }
        self.table.free_reg(bin.lhs(), rs1);
        self.table.free_reg(bin.rhs(), rs2);
        Some(rd)
    }

    pub fn build_branch(&mut self, asm_bb: &mut AsmLocal, val: Value, branch: &values::Branch, rd: Option<Reg>) -> Option<Reg> {

        let func_data = self.ctx.program.func(self.ctx.function.unwrap());
        let get_name = |bb| {
            func_data.dfg().bb(bb).name().as_ref().unwrap()[1..].to_string().replace('%', "")
        };

        let name = get_name(branch.false_bb());
        println!("{}",name);
        println!("{}",name.replace("%", ""));

        let cond = self.move_inst(asm_bb, branch.cond(), None);
        asm_bb.beqz_inst(cond, get_name(branch.false_bb()));
        asm_bb.J_inst(get_name(branch.true_bb()));
        self.table.free_reg(branch.cond(), cond);
        None
    }

    pub fn build_jump(&mut self, asm_bb: &mut AsmLocal, val: Value, jump: &values::Jump, rd: Option<Reg>) -> Option<Reg> {
        
        let func_data = self.ctx.program.func(self.ctx.function.unwrap());
        let get_name = |bb| {
            func_data.dfg().bb(bb).name().as_ref().unwrap()[1..].to_string().replace('%', "")
        };

        asm_bb.J_inst(get_name(jump.target()));
        None
    }

    pub fn build_store(&mut self, asm_bb: &mut AsmLocal, val: Value, store: &values::Store, rd: Option<Reg>) -> Option<Reg> {
        // let rd = self.table.alloc_reg(val, rd);
        let rs = self.move_inst(asm_bb, store.value(), None);
        let dst_val = store.dest();
        
        println!("build_Store in reg {}, store_value {:?}", rs, store.value());
        if dst_val.is_global() {
            println!("global val = {:?}", dst_val);
            let var_name = self.find_global_name(dst_val);
            asm_bb.la_inst("t0", var_name);
            asm_bb.sw_inst(rs, 0, "t0");
            self.table.free_reg(store.value(), rs);
        } else if self.offset.get(&dst_val) != None {
            let imm = self.offset.get(&dst_val).unwrap().clone() as i32;
            println!("offset = {}", imm);
            asm_bb.sw_inst(rs, imm, "sp");
            self.table.free_reg(store.value(), rs);
        } else {
            let rd = self.move_inst(asm_bb, dst_val, None);
            asm_bb.sw_inst(rs, 0, rd);
            self.table.free_reg(store.value(), rs);
            self.table.free_reg(store.dest(), rd);
        }
        self.print_state();
        None


    }

    pub fn build_load(&mut self, asm_bb: &mut AsmLocal, val: Value, load: &values::Load, rd: Option<Reg>) -> Option<Reg> {
        let src_val = load.src();
        let rd = self.table.alloc_reg(val, rd);

        if src_val.is_global() {
            println!("global val = {:?}",src_val);
            let var_name = self.find_global_name(src_val);
            asm_bb.la_inst(rd, var_name);
            asm_bb.lw_inst(rd, 0 ,rd);
        } else if self.offset.get(&src_val) != None {
            let imm = self.offset.get(&src_val).unwrap().clone() as i32;
            asm_bb.lw_inst(rd, imm, "sp");
        } else {
            let rs = self.move_inst(asm_bb, src_val, None);
            asm_bb.lw_inst(rd, 0, rs);
            self.table.free_reg(src_val, rs);
        }
        
        Some(rd)
    }

    pub fn build_call(&mut self, asm_bb: &mut AsmLocal, val: Value, call: &values::Call, rd: Option<Reg>) -> Option<Reg> {

        let func_data = self.ctx.program.func(self.ctx.function.unwrap());
        // pass args
        call.args().iter().enumerate().for_each(|(i, &para)| {
            let kind = func_data.dfg().value(para).kind();
            let reg = match i < 8 {
               true => PARA_REG[i],
               false => "t0",
            };
            match kind {
                ValueKind::Integer(int) => asm_bb.li_inst(reg, int.value()),
                _ => asm_bb.lw_inst(reg, self.offset.get(&para).unwrap().clone() as i32, "sp"),
            }
            if i >= 8 {
                asm_bb.sw_inst("t0", (i - 8) as i32 * 4, "sp");
            }
        });

        // save all values 

        // caculate the number of allocated reg
        let mut off = 0;
        let regs: Vec<Reg> = self.table.val2reg.values().map(|reg| *reg).collect();
        let num = regs.len() as i32;
        if num != 0 {
            asm_bb.addi_inst("sp", "sp", -num * 4);
            for i in (0..(self.paranum as i32/4)) {
                asm_bb.lw_inst("t0", i as i32 * 4 + num * 4, "sp");
                asm_bb.sw_inst("t0", i as i32 * 4, "sp");
            }
        
            for reg in regs {
                asm_bb.sw_inst(reg, off + self.paranum as i32, "sp");
                off += 4;
            }
        }

        // Call 
        let func_data = self.ctx.program.func(call.callee());
        let func_name = func_data.name()[1..].to_string();
        asm_bb.call_inst(func_name.to_string());

        // Restore 
        let regs: Vec<Reg> = self.table.val2reg.values().map(|reg| *reg).collect();
        let num = regs.len() as i32;
        if num != 0 {
            off = 0;

            for reg in regs {
                asm_bb.lw_inst(reg, off + self.paranum as i32, "sp");
                off += 4;
            }
            asm_bb.addi_inst("sp", "sp", num* 4);
        }
        
        let func_data = self.ctx.program.func(self.ctx.function.unwrap());
        let value_data = func_data.dfg().value(val);
        if !value_data.used_by().is_empty() {
            let rd = self.table.alloc_reg(val, rd);
            asm_bb.mv_inst(rd, "a0");
            return Some(rd)
        }
        None
    }

    pub fn build_get_ptr(&mut self, asm_bb: &mut AsmLocal, val : Value, getptr: &values::GetPtr, dst: Option<Reg>) -> Option<Reg>{
        let src_val = getptr.src();
        let src_kind = {
            if src_val.is_global() {
                let value_data = self.ctx.program.borrow_value(src_val);
                value_data.ty().kind().clone()
            } else {
                let func_data = self.ctx.program.func(self.ctx.function.unwrap());
                let value_data = func_data.dfg().value(src_val);
                value_data.ty().kind().clone()
            }
        };

        let base_size = match src_kind {
            TypeKind::Array(base,_ ) => base.size() as usize,
            TypeKind::Pointer(base) => base.size() as usize,
            _ => panic!("Pointer expected"),
        };
        let index = getptr.index();
        let rd;

        if src_val.is_global() {
            let idx = self.move_inst(asm_bb, index, None);
            let off = self.table.alloc_reg(val, None);
            asm_bb.muli_inst(off, idx, base_size as i32);
            self.table.free_reg(index, idx);
            self.table.free_reg(val, off);
            asm_bb.la_inst("t0", self.find_global_name(src_val));
            rd = self.table.alloc_reg(val, dst);
            asm_bb.add_inst(rd, "t0", off);
        } else if self.offset.get(&src_val) != None {
            let idx = self.move_inst(asm_bb, index, None);
            let off = self.table.alloc_reg(val, None);
            asm_bb.muli_inst(off, idx, base_size as i32);
            self.table.free_reg(index, idx);
            self.table.free_reg(val, off);
            let imm = self.offset.get(&src_val).unwrap().clone() as i32;
            asm_bb.addi_inst("t0", "sp", imm);
            rd = self.table.alloc_reg(val, dst);
            asm_bb.add_inst(rd, "t0", off);
        } else { 
            let rs = self.move_inst(asm_bb, src_val, None);
            let idx = self.move_inst(asm_bb, index, None);
            let off = self.table.alloc_reg(val, None);
            asm_bb.muli_inst(off, idx, base_size as i32);
            self.table.free_reg(index, idx);
            self.table.free_reg(val, off);
            rd = self.table.alloc_reg(val, dst);
            asm_bb.add_inst(rd, off, rs);
            self.table.free_reg(src_val, rs);
        }

        Some(rd)

    }


    pub fn build_getelem_ptr(&mut self, asm_bb: &mut AsmLocal, val : Value, getelem : &values::GetElemPtr, dst: Option<Reg>) -> Option<Reg>{
        let src_val = getelem.src();
        let src_kind = {
            if src_val.is_global() {
                let value_data = self.ctx.program.borrow_value(src_val);
                value_data.ty().kind().clone()
            } else {
                let func_data = self.ctx.program.func(self.ctx.function.unwrap());
                let value_data = func_data.dfg().value(src_val);
                value_data.ty().kind().clone()
            }
        };

        let base_size = match src_kind {
            TypeKind::Pointer(base) => match base.kind() {
                TypeKind::Array(base,_ ) => base.size() as usize,
                _ => panic!("Pointer expected"),
            },
            _ => panic!("Pointer expected"),
        };
        let index = getelem.index();
        let rd;

        if src_val.is_global() {
            let idx = self.move_inst(asm_bb, index, None);
            let off = self.table.alloc_reg(val, None);
            asm_bb.muli_inst(off, idx, base_size as i32);
            self.table.free_reg(index, idx);
            self.table.free_reg(val, off);
            asm_bb.la_inst("t0", self.find_global_name(src_val));
            rd = self.table.alloc_reg(val, dst);
            asm_bb.add_inst(rd, "t0", off);
        } else if self.offset.get(&src_val) != None {
            let idx = self.move_inst(asm_bb, index, None);
            let off = self.table.alloc_reg(val, None);
            asm_bb.muli_inst(off, idx, base_size as i32);
            self.table.free_reg(index, idx);
            self.table.free_reg(val, off);
            let imm = self.offset.get(&src_val).unwrap().clone() as i32;
            asm_bb.addi_inst("t0", "sp", imm);
            rd = self.table.alloc_reg(val, dst);
            asm_bb.add_inst(rd, "t0", off);
        } else { 
            let rs = self.move_inst(asm_bb, src_val, None);
            let idx = self.move_inst(asm_bb, index, None);
            let off = self.table.alloc_reg(val, None);
            asm_bb.muli_inst(off, idx, base_size as i32);
            self.table.free_reg(index, idx);
            self.table.free_reg(val, off);
            rd = self.table.alloc_reg(val, dst);
            asm_bb.add_inst(rd, off, rs);
            self.table.free_reg(src_val, rs);
        }

        Some(rd)

    }

    pub fn build_global_alloc(&mut self, asm_prog: &mut AsmProgram, val: Value) -> AsmGlobalDef {
        // let func_data = self.ctx.program.(self.ctx.function.unwrap());
        // let value_data = func_data.dfg().value(val);

        let value_data = self.ctx.program.borrow_value(val);
        let var_name = self.find_global_name(val);
        println!("var_name = {:?}", var_name);

        
        println!("global val = {:?}", val);

        let ValueKind::GlobalAlloc(alloc) = value_data.kind() else {
            panic!("GlobalAlloc expected");
        };
        let mut vec = Vec::new();
        

        let data = self.ctx.program.borrow_value(alloc.init());
        let kind = data.kind();
        match kind {
            ValueKind::Integer(int) => vec.push(InitVal::Word(int.value())),
            ValueKind::ZeroInit(_) => {
                let size = match value_data.ty().kind() {
                    TypeKind::Pointer(base) => base.size() as usize,
                    _ => panic!("Pointer expected"),
                };
                vec.push(InitVal::Zero(size));
            }
            ValueKind::Aggregate(arr) => {
                let mut arr_vec: Vec<InitVal> = Vec::new();
                self.build_aggregate(asm_prog, arr, &mut arr_vec);
                vec.push(InitVal::Array(arr_vec));
            }
            _ => panic!("Unsupport"),
        }

        AsmGlobalDef {
            label: Label::new(var_name),
            init_val: vec,
        }
    }



    pub fn build_aggregate(&mut self, asm_prog: &mut AsmProgram, arr: &values::Aggregate, vec: &mut Vec<InitVal>) {
        arr.elems().iter().for_each(|&val| {
            let func_data = self.ctx.program.borrow_value(val);
            let kind = func_data.kind();

            match kind {
                ValueKind::Integer(int) => vec.push(InitVal::Word(int.value())),
                ValueKind::Aggregate(agg) => {
                    self.build_aggregate(asm_prog, agg, vec);
                }
                _ => panic!("Unsupport"),
            }
        });
    }
}

pub fn generate_random_string(length: usize) -> String {
    let mut rng = rand::thread_rng();
    let dist = Uniform::from(b'a'..=b'z');
    let mut result = String::with_capacity(length);
    for _ in 0..length {
        let c = dist.sample(&mut rng) as char;
        result.push(c);
    }
    result
}
