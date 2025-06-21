use koopa::ir::ValueKind;

use crate::ast_df::BType;

use super::env::Env;

pub fn check_ir(env: &mut Env, ret_type: BType) {
    // check the last inst of the blocks           
    let func = env.ctx.func.expect("No function in context");
    let func_data = env.ctx.program.func_mut(func);
    
    let err_bbs = func_data.layout().bbs().iter().filter(|node| {
        match node.1.insts().back_key() {
            Some(inst) => {
                let inst_data = func_data.dfg().value(inst.clone());
                match inst_data.kind() {
                    ValueKind::Return(_) => false,
                    ValueKind::Jump(_) => false,
                    ValueKind::Branch(_) => false,
                    _ => true,
                }
            }
            None => true
        }
    }).collect::<Vec<_>>();

    let err_bbs = err_bbs.iter().map(|node| node.0.clone()).collect::<Vec<_>>();
    
    err_bbs.iter().for_each(|bb| {
        env.ctx.block = Some(bb.clone());

        match ret_type {
            BType::Int => {
                let num = env.ctx.create_int_inst(0);
                env.ret_inst(num);
            },
            BType::Void => {
                env.ret_void_inst();
            },
        }
    });
}