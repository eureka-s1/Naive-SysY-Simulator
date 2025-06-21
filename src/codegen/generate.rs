
use std::collections::HashMap;

use crate::codegen::instruction::{idx2reg, Reg};

use super::asm::{AsmProgram, AsmGlobal, AsmLocal, Section};
use super::instruction::Inst;
use super::label::Label;
use super::env::{Context, Env};

use koopa::ir::entities::ValueData;
use koopa::ir::*;

// generate assembly code from IR
pub trait GenerateAsm {
    type AsmTarget;

    fn generate(&self, env: &mut Env, asm: &mut Self::AsmTarget);
}

impl GenerateAsm for Program {
    type AsmTarget = AsmProgram;

    fn generate(&self, env: &mut Env, asm: &mut AsmProgram) {
        // Global Alloc 
        for &global in self.inst_layout() {
            let global_def = env.build_global_alloc(asm, global);
            asm.push_globaldef(global_def);
        }

        // Func
        for &func in self.func_layout() {
            // skip builtin function

            if self.func(func).layout().entry_bb() == None {
                continue;
            }

            // store the current function and generate code recursively
            env.ctx.function = Some(func);
            
            let func_data = self.func(func);
            let mut asm_func = AsmGlobal::new(Section::Text, Label::new(func_data.name().to_string()));

            // ToDo : calculate offset 
            {
                env.frame_size = 0;
                env.offset = HashMap::new();
                let values = func_data.layout().bbs().nodes().flat_map(|block| 
                    block.insts().keys().map(|&val| val)).collect::<Vec<_>>();
                
                // alloc var
                values.iter().for_each(|&val| {
                    if let ValueKind::Alloc(_) = func_data.dfg().value(val).kind() {
                        env.offset.insert(val, env.frame_size as usize);

                        let func_data = self.func(func);
                        let kind = func_data.dfg().value(val).ty().kind();
                        let size = if let TypeKind::Pointer(base) = kind {
                            base.size()
                        } else {
                            panic!("Unexpected type kind");
                        };
                        env.frame_size += size as i32;
                    }
                });

                // temporary function call
                values.iter().for_each(|&val| {
                    let used_by = func_data.dfg().value(val).used_by();
                    if used_by.len() == 1 {
                        let &user = used_by.iter().next().unwrap();
                        if let ValueKind::Call(..) = func_data.dfg().value(user).kind() {
                            env.offset.insert(val, env.frame_size as usize);
                            env.frame_size += 4;
                        }
                    }
                });

                env.frame_size += 4; // return address

                let max_arg_num = values.iter()
                    .map(|&val| match func_data.dfg().value(val).kind() {
                        ValueKind::Call(call) => call.args().len() as usize,
                        _ => 0,
                    })
                    .max()
                    .unwrap_or(0);
            
                if max_arg_num > 8 {
                    env.paranum = 4 * (max_arg_num - 8);
                    env.frame_size += env.paranum as i32;
                    env.offset.values_mut().for_each(|offset| *offset += env.paranum);
                }

                env.frame_size = (env.frame_size + 15) / 16 * 16;
            }

            func_data.generate(env, &mut asm_func);
            asm.push_global(asm_func);
        }
    }
}

pub const PARA_REG: [Reg; 8] = [
    "a0", "a1", "a2", "a3", 
    "a4", "a5", "a6", "a7",
];

impl GenerateAsm for FunctionData {
    type AsmTarget = AsmGlobal;

    fn generate(&self, env: &mut Env, asm_func: &mut AsmGlobal) {
        // generate code for each basic block in the function

        println!("enter {}", self.name());

        // alloc parameters values to regs
        self.params().iter().take(8).enumerate().for_each(|(i, &val)| {
            println!("parameter: {:?}", val);
            env.table.alloc_reg(val, Some(PARA_REG[i]));
        });

        let mut is_entry = true;
        for (&bb, node) in self.layout().bbs() {
            // generate a unique name (label) for each basic block
            let bb_data = self.dfg().bb(bb);

            let label = Label::new(format!("{}", 
                // self.name().to_string(), 
                bb_data.name().clone().unwrap()
            ));
            let mut asm_bb = AsmLocal::new(Some(label));

            env.ctx.block = Some(bb);

            // alloc stack frame
            if is_entry {
                let size = env.frame_size;
                asm_bb.addi_inst("sp", "sp", -size);
                asm_bb.sw_inst("ra", size - 4, "sp");
            }

            // generate code for each instruction in the basic block
            for &inst in node.insts().keys() {
                println!("intst: {:?}", inst);
                let value_data = self.dfg().value(inst);
                // value_data.generate(env, &mut asm_bb);
                env.build_inst(&mut asm_bb, inst,None, value_data.kind());
            }

            is_entry = false;
            asm_func.push_local(asm_bb);
        }
    }
}

impl GenerateAsm for ValueData {
    type AsmTarget = AsmLocal;

    fn generate(&self, env: &mut Env, asm_bb: &mut AsmLocal) {
        // match self.kind() {
        //     ValueKind::Integer(int) => {},
        //     ValueKind::Alloc(alloc) => {},
        //     ValueKind::Load(load) => todo!(),
        //     ValueKind::Store(store) => todo!(),
        //     ValueKind::Binary(binary) => todo!(),
        //     ValueKind::Branch(branch) => todo!(),
        //     ValueKind::Jump(jump) => todo!(),
        //     ValueKind::Call(call) => todo!(),
        //     ValueKind::Return(ret) => env.build_ret(ret.value()),
        //     ValueKind::GetPtr(getptr) => todo!(),
        //     ValueKind::GetElemPtr(getelemptr) => todo!(),
        //     _ => panic!("Unsupported value kind"),
        // }
    }
}
