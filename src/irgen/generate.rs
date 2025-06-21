// generating koopa IR from AST 
// Koopa IR

use core::panic;
use koopa::ir::*;
use koopa::ir::builder_traits::*;

use crate::ast_df::*;
use crate::irgen::array::global_const_init;
use crate::irgen::array::global_var_init;
use crate::irgen::array::local_const_init;
use crate::irgen::array::local_var_init;
use super::env::Env;
use super::eval::EvalExp;
use super::opt;
use super::builtin;
use super::array;

use rand::Rng;
use rand::distributions::Alphanumeric;

pub enum ControlFlow {
    Break,
    Continue,
    Return,
}

type CFType = Result<(), ControlFlow>;

pub trait GenerateIR {
    type RetType;

    fn generate(&self, env: &mut Env) -> Self::RetType;
}

impl GenerateIR for CompUnit {
    type RetType = ();

    fn generate(&self, env: &mut Env) -> () {
        // global scope
        env.scope.enter_scope();
        builtin::builtin_decl(env);

        for global_item in &self.items {
            let _ = global_item.generate(env);
        }

        env.scope.exit_scope();
    }   
}

impl GenerateIR for GlobalItem {
    type RetType = ();

    fn generate(&self, env: &mut Env) -> () {
        match self {
            GlobalItem::FuncDef(func_def) => {
                let _ = func_def.generate(env);
            }
            GlobalItem::Decl(decl) => {
                decl.generate(env);
            }
        }
    }
}

impl FuncDef {
    pub fn new_func(&self, env: &mut Env) ->  (Vec<(String, Option<Vec<i32>>)>, Function) {
        let ret_type = match self.func_type {
            BType::Int  => Type::get_i32(),
            BType::Void => Type::get_unit(),
        };

        let func_name = format!("{}{}", "@", self.ident);
        
        let params = self.params.iter().map(|param| {
            let param_name = format!("{}{}", "@", param.id);
            let dims = match &param.dims {
                None => None,
                Some(dims) => 
                    Some(dims.iter().map(|dim| 
                        dim.eval(env)
                    ).collect::<Vec<_>>()),
            };
            (param_name, dims)
        }).collect::<Vec<_>>();

        let params_type = params.iter().map(|param| {
            let param_name = format!("{}", param.0.clone());
            let param_type = match &param.1 {
                None => Type::get_i32(),
                Some(dims) => array::get_array_p(dims),
            };
            (Some(param_name), param_type)
        }).collect::<Vec<_>>();
        

        let func = env.ctx.program.new_func(FunctionData::with_param_names(
            func_name, 
            params_type,
            ret_type,
        ));

        (params, func)
    }
}

impl GenerateIR for FuncDef {
    type RetType = CFType;
    
    fn generate(&self, env: &mut Env) -> CFType {
        /////////////////////////////////////
        // generate a new function in KoopaIR

        // let func_name = format!("{}{}", "@", self.ident);
        let (params, func) = self.new_func(env);
               
        env.ctx.func = Some(func);
        env.scope.insert_func(&self.ident, func);
        let block = env.ctx.create_block(Some("entry".to_string()));
        env.ctx.block = Some(block);

        /////////////////////////////////////
        // enter a new function scope
        env.scope.enter_scope();

        let func_data = env.ctx.program.func_mut(func);
        let param_to_val = func_data.params().iter().zip(params.iter()).map(|arg| {
            let (val, (ident, dims)) = arg;
            let param_val = val.clone();
            let param_name = ident.clone().chars().skip(1).collect();
            (param_val, param_name, dims)
        }).collect::<Vec<_>>();

        for (val, ident, dims) in param_to_val {
            match dims {
                None => {
                    env.alloc_var(&ident);
                    env.store_var(&ident, val);
                },
                Some(dims) => {
                    env.func_alloc_pointer(&ident, array::get_array_p(dims), dims);
                    env.store_var(&ident, val);
                }
            }
        }

        // generate block recursively
        // terminate the function if Return inst occurs in this block
        let _ = self.block.generate(env);
        // ToDo: check the last inst of the blocks   
        opt::check_ir(env, self.func_type.clone());
        
        // exit the function scope        
        env.scope.exit_scope();

        Ok(())

    }
}

impl GenerateIR for FuncCall {
    type RetType = Value;

    fn generate(&self, env: &mut Env) -> Value {
        let func_name = &self.id;
        let func = env.scope.lookup_func(func_name).expect("Function not found");
        let params = self.args.iter().map(|param| {
            let param_val = param.generate(env);
            param_val
        }).collect::<Vec<_>>();
        env.call_inst(func, params)
    }
}

impl GenerateIR for Block {
    type RetType = CFType;

    fn generate(&self, env: &mut Env) -> CFType {

        // enter a new scope
        env.scope.enter_scope();

        for item in &self.items {
            let ret_val = item.generate(env);
            match ret_val {
                Ok(_) => (),
                Err(c) => {
                    env.scope.exit_scope();
                    return Err(c);
                }
            }
        }
        // exit scope
        env.scope.exit_scope();

        Ok(())
    }
}

impl GenerateIR for BlockItem {
    type RetType = CFType;

    fn generate(&self, env: &mut Env) -> CFType {
        
        match self {
            BlockItem::Decl(decl) => {
                decl.generate(env);
                Ok(())
            }
            BlockItem::Stmt(stmt) => {
                let _ = stmt.generate(env)?;
                Ok(())
            }
        }
    }
}


/////////////////////
//    Stmt Part    //
/////////////////////

impl GenerateIR for Stmt {
    type RetType = CFType;
    fn generate(&self, env: &mut Env) -> CFType {
        match self {
            Stmt::Empty => Ok(()),
            Stmt::Exp(exp) => {
                exp.generate(env);
                Ok(())
            }
            Stmt::Block(block) => {
                let _ = block.generate(env)?;
                Ok(())
            }
            Stmt::Assign(assign) => {
                assign.generate(env);
                Ok(())
            }
            Stmt::Return(ret) => {
                ret.generate(env);
                Err(ControlFlow::Return)
            },
            Stmt::If(r#if) => {
                let _ = r#if.generate(env);
                Ok(())
            },
            Stmt::While(while_stmt) => {
                let _ = while_stmt.generate(env);
                Ok(())
            }
            Stmt::Break => {
                let (_, exit) = env.loopstack.top();
                env.jump_inst(exit);
                Err(ControlFlow::Break)
            }
            Stmt::Continue => {
                let (entry, _) = env.loopstack.top();
                env.jump_inst(entry);
                Err(ControlFlow::Continue)
            }
        }
    }
}


impl GenerateIR for While {
    type RetType = CFType;
    fn generate(&self, env: &mut Env) -> CFType {
        let While { cond, stmt } = self;
        let cond_block = env.ctx.create_block(Some("cond".to_string()));
        let body_block = env.ctx.create_block(Some("body".to_string()));
        let end_block = env.ctx.create_block(Some("end".to_string()));

        env.loopstack.push(&cond_block,&end_block);

        // jump to cond block
        env.jump_inst(cond_block);
        env.ctx.block = Some(cond_block);

        let cond_val = cond.generate(env);
        // branch to body block if cond is true
        env.branch_inst(cond_val, body_block, end_block);
        
        // body block
        env.ctx.block = Some(body_block);
        let ret_val = stmt.generate(env);
        // return, break, while
        match ret_val {
            Ok(()) => env.jump_inst(cond_block),
            Err(_) => (),
        }

        // end block
        env.ctx.block = Some(end_block);

        env.loopstack.pop();
        
        Ok(())
    }
}

impl GenerateIR for If {
    type RetType = CFType;

    fn generate(&self, env: &mut Env) -> CFType {
        let If { cond, stmt, else_stmt } = self;
        let cond_block = env.ctx.create_block(Some("cond".to_string()));
        let then_block = env.ctx.create_block(Some("then".to_string()));
        let else_block = env.ctx.create_block(Some("else".to_string()));
        let end_block = env.ctx.create_block(Some("end".to_string()));

        // Cond Part 
        // jump to cond block
        env.jump_inst(cond_block);
        env.ctx.block = Some(cond_block);

        let cond_val = cond.generate(env);

        match else_stmt {
            Some(_) => env.branch_inst(cond_val, then_block, else_block),    
            None => env.branch_inst(cond_val, then_block, end_block),
        };

        // Then Part
        env.ctx.block = Some(then_block);
        let ret_val = stmt.generate(env);
        match ret_val {
            Ok(()) =>  env.jump_inst(end_block),
            Err(_) => (),
        };

        // Else Part
        match else_stmt {
            Some(else_stmt) => {
                env.ctx.block = Some(else_block);
                let ret_val = else_stmt.generate(env);
                match ret_val {
                    Ok(()) =>  env.jump_inst(end_block),
                    Err(_) => (),
                }
            },
            None => {
                env.ctx.remove_bb(else_block);
            }
        }

        // End Part
        env.ctx.block = Some(end_block);

        Ok(())
    }
}

impl GenerateIR for Assign {
    type RetType = ();

    fn generate(&self, env: &mut Env) {
        let Assign { lval, exp } = self;
        match lval {
            LVal::Ident(ident) => {
                let value = exp.generate(env);
                env.store_var(&ident, value);
            },
            LVal::Array(ident, dims,) => {
                // Very important
                let is_pointer = env.scope.lookup_is_pointer(ident).unwrap();
                let mut addr = match is_pointer {
                    true => env.load_var(ident),
                    false => env.scope.lookup_var_addr(ident).unwrap(),
                };

                let mut iter = dims.iter();
                if let Some(dim) = iter.next() {
                    let index = dim.generate(env);
                    match is_pointer {
                        true => addr = env.get_ptr_inst(addr, index),
                        false => addr = env.get_elem_inst(addr, index),
                    }
                }

                for dim in iter {
                    let dim_size = dim.generate(env);
                    addr = env.get_elem_inst(addr, dim_size);
                }

                let value = exp.generate(env);
                env.store_val_by_addr(addr, value);

            }
        }
    }
}

impl GenerateIR for Return {
    type RetType = ();

    fn generate(&self, env: &mut Env) {
        let Return { exp } = self;
        match exp {
            Some(exp) => {
                let value = exp.generate(env);
                env.ret_inst(value);
            },
            None => {
                env.ret_void_inst();
            },
        }
    }
}


//////////////////////
// Declaration Part //
//////////////////////
 
impl GenerateIR for Decl {
    type RetType = ();

    fn generate(&self, env: &mut Env) -> () {
        match self {
            Decl::Const(const_decl) => const_decl.generate(env),
            Decl::Var(var_decl) => var_decl.generate(env),
        }
    }
}

impl GenerateIR for ConstDecl {
    type RetType = ();

    fn generate(&self, env: &mut Env) {
        for const_def in &self.const_defs {
            match self.is_global {
                true => global_const_decl_gen(env, const_def),
                false => {
                    let _ = const_def.generate(env);
                }
            }
        }
    }
}

pub fn global_const_decl_gen(env: &mut Env, const_def: &ConstDef) {
    let ConstDef { ident, init_val, dims } = const_def;
    match dims {
        None => {
            let num = init_val.generate(env);
            env.create_const_var(ident, num);
        }
        Some(dims) => {
            let dims_size = dims.iter().map(|dim| dim.eval(env)).collect::<Vec<_>>();
            
            let val = global_const_init(env, &dims_size, init_val);
            env.alloc_global_array(ident, &dims_size, val); 
        }
    };
}



impl GenerateIR for ConstDef {
    type RetType = ();

    fn generate(&self, env: &mut Env) {
        let ConstDef { ident, init_val, dims } = self;
        match dims {
            None => {
                let num = init_val.generate(env);
                env.create_const_var(ident, num);
            },
            Some(dims) => {
                // Only Local here
                let dims_size = dims.iter().map(|dim| dim.eval(env)).collect::<Vec<_>>();
                
                // A series of store inst
                let vec = local_const_init(env, &dims_size, init_val);

                env.alloc_array(ident, &dims_size, Some(vec));
            }
        }
    }
}

impl GenerateIR for ConstInitVal {
    type RetType = i32;

    fn generate(&self, env: &mut Env) -> i32 {
        match self {
            ConstInitVal::ConstExp(exp) => exp.eval(env),
            ConstInitVal::InitList(..) => {
                panic!("Not support");
            }
        }
    }
}

impl GenerateIR for VarDecl {
    type RetType = ();
    fn generate(&self, env: &mut Env) -> () {
        for var_def in &self.defs {
            match self.is_global {
                true => global_var_decl_gen(env, var_def),
                false => {
                    let _ = var_def.generate(env);
                },
            }
        }
    }
}

// generate global var declaration
pub fn global_var_decl_gen(env: &mut Env, var_def: &VarDef) {
    let VarDef { ident, init_val, dims } = var_def;
    match dims {
        None => {
            let num = match init_val{
                None => 0,
                Some(init_val) => match init_val {
                    InitVal::Exp(exp) => exp.eval(env),
                    InitVal::InitList(_) => panic!("Global var can't be initialized by list"),
                }
            };
            let _ = env.alloc_global_var(ident, num);
        }
        Some(dims) => {
            let dims_size = dims.iter().map(|dim| dim.eval(env)).collect::<Vec<_>>();
            match init_val {
                Some(init_val) => {
                    let val = global_var_init(env, &dims_size, init_val);
                    env.alloc_global_array(ident, &dims_size, val);
                }
                None => {
                    let ty = array::get_array(&dims_size);
                    let val = env.ctx.program.new_value().zero_init(ty);
                    env.alloc_global_array(ident, &dims_size, val);
                }
            } 
        }
    };
}


impl GenerateIR for VarDef {
    type RetType = ();
    fn generate(&self, env: &mut Env) -> () {
        let VarDef { ident, init_val, dims } = self;
        match dims {
            None => {
                match init_val {
                    // each var alloc only once
                    Some(init_val) => {
                        // alloc and store
                        let val = init_val.generate(env);
                        let _ = env.alloc_var(ident);
                        env.store_var(ident, val);
                    },
                    None => {
                        // alloc only
                        let _ = env.alloc_var(ident);
                    },
                }
            }
            Some(dims) => {
                // Only Local here
                let dims_size = dims.iter().map(|dim| dim.eval(env)).collect::<Vec<_>>();
                match init_val {
                    Some(init_val) => {
                        let vec = local_var_init(env, &dims_size, init_val);
                        env.alloc_array(ident, &dims_size, Some(vec));
                    }
                    None => {
                        env.alloc_array(ident, &dims_size, None);
                    }
                } 
            }
        }
    }
}

impl GenerateIR for InitVal {
    type RetType = Value;

    fn generate(&self, env: &mut Env) -> Value {
        match self {
            InitVal::Exp(exp) => exp.generate(env),
            InitVal::InitList(init_list) => todo!(),
        }
    }
}


/////////////////////
// Expression Part //
/////////////////////

impl GenerateIR for Exp {
    type RetType = Value;
    fn generate(&self, env: &mut Env) -> Value {
        let Exp::LOrExp(l_or_exp) = self;
        l_or_exp.generate(env)
    }
}

impl GenerateIR for LOrExp {
    type RetType = Value;

    fn generate(&self, env: &mut Env) -> Self::RetType {
        match self {
            LOrExp::LAnd(l_and_exp) => l_and_exp.generate(env),
            LOrExp::LOrLAnd(l_or_exp, l_and_exp) => {

                let zero = env.ctx.create_int_inst(0);
                let one = env.ctx.create_int_inst(1);
                // ShortCircuit: SimleImpl
                
                let cond_bb = env.ctx.create_block(Some("cond".to_string()));
                let then_bb = env.ctx.create_block(Some("then".to_string()));
                let end_bb = env.ctx.create_block(Some("end".to_string()));

                env.jump_inst(cond_bb);
                env.ctx.block = Some(cond_bb);

                // tmporarily store the result of l_or_exp
                let tmp_id = format!("tmpvar_{}_{}", env.ctx.block_count, generate_random_string(10));
                let tmp_var = env.alloc_var(&tmp_id);
                env.store_var(&tmp_id, one);

                let mut l_or_val = l_or_exp.generate(env);
                l_or_val = env.ctx.insert_bi_inst(BinaryOp::NotEq, l_or_val, zero);
                env.branch_inst(l_or_val, end_bb, then_bb);

                env.ctx.block = Some(then_bb);
                let mut l_and_val = l_and_exp.generate(env);
                l_and_val = env.ctx.insert_bi_inst(BinaryOp::NotEq, l_and_val, zero);
                env.store_var(&tmp_id, l_and_val);
                env.jump_inst(end_bb);

                env.ctx.block = Some(end_bb);
                env.load_var(&tmp_id)
            },
        }
    }
}

impl GenerateIR for LAndExp {
    type RetType = Value;

    fn generate(&self, env: &mut Env) -> Self::RetType {
        match self {
            LAndExp::Eq(eq_exp) => eq_exp.generate(env),
            LAndExp::LAndEq(l_and_exp, eq_exp) => {
                // let zero = env.ctx.create_int_inst(0);
                // let one = env.ctx.create_int_inst(1);

                // let mut l_and_val = l_and_exp.generate(env);
                // l_and_val = env.ctx.insert_bi_inst(BinaryOp::NotEq, l_and_val, zero);

                // let mut eq_val = eq_exp.generate(env);
                // eq_val = env.ctx.insert_bi_inst(BinaryOp::NotEq, eq_val, zero);

                // env.ctx.insert_bi_inst(BinaryOp::And, l_and_val, eq_val)

                let zero = env.ctx.create_int_inst(0);
                
                let cond_bb = env.ctx.create_block(Some("cond".to_string()));
                let then_bb = env.ctx.create_block(Some("then".to_string()));
                let end_bb = env.ctx.create_block(Some("end".to_string()));

                env.jump_inst(cond_bb);
                env.ctx.block = Some(cond_bb);

                // tmporarily store the result of l_or_exp
                let tmp_id = format!("tmpvar_{}_{}", env.ctx.block_count, generate_random_string(10));
                let tmp_var = env.alloc_var(&tmp_id);
                env.store_var(&tmp_id, zero);

                let mut l_and_val = l_and_exp.generate(env);
                l_and_val = env.ctx.insert_bi_inst(BinaryOp::NotEq, l_and_val, zero);
                env.branch_inst(l_and_val, then_bb, end_bb);

                env.ctx.block = Some(then_bb);
                let mut eq_val = eq_exp.generate(env);
                eq_val = env.ctx.insert_bi_inst(BinaryOp::NotEq, eq_val, zero);
                env.store_var(&tmp_id, eq_val);
                env.jump_inst(end_bb);

                env.ctx.block = Some(end_bb);
                env.load_var(&tmp_id)
            },
        }
    }
}

impl GenerateIR for EqExp {
    type RetType = Value;

    fn generate(&self, env: &mut Env) -> Self::RetType {
        match self {
            EqExp::Rel(rel_exp) => rel_exp.generate(env),
            EqExp::EqRel(eq_exp, eq_op, rel_exp) => {
                let eq_val = eq_exp.generate(env);
                let rel_val = rel_exp.generate(env);
                match eq_op {
                    EqOp::Eq => env.ctx.insert_bi_inst(BinaryOp::Eq, eq_val, rel_val),
                    EqOp::Neq => env.ctx.insert_bi_inst(BinaryOp::NotEq, eq_val, rel_val),
                }
            },
        }
    }
}

impl GenerateIR for RelExp {
    type RetType = Value;

    fn generate(&self, env: &mut Env) -> Self::RetType {
        match self {
            RelExp::Add(add_exp) => add_exp.generate(env),
            RelExp::RelAdd(rel_exp, rel_op, add_exp) => {
                let rel_val = rel_exp.generate(env);
                let add_val = add_exp.generate(env);
                match rel_op {
                    RelOp::Lt => env.ctx.insert_bi_inst(BinaryOp::Lt, rel_val, add_val),
                    RelOp::Gt => env.ctx.insert_bi_inst(BinaryOp::Gt, rel_val, add_val),
                    RelOp::Le => env.ctx.insert_bi_inst(BinaryOp::Le, rel_val, add_val),
                    RelOp::Ge => env.ctx.insert_bi_inst(BinaryOp::Ge, rel_val, add_val),
                }
            },
        }
    }
}

impl GenerateIR for AddExp {
    type RetType = Value;

    fn generate(&self, env: &mut Env) -> Self::RetType {
        match self {
            AddExp::Mul(mul_exp) => mul_exp.generate(env),
            AddExp::AddMul(add_exp, add_op ,mul_exp) => {
                let add_val = add_exp.generate(env);
                let mul_val = mul_exp.generate(env);
                match add_op {
                    AddOp::Add => env.ctx.insert_bi_inst(BinaryOp::Add, add_val, mul_val),
                    AddOp::Sub => env.ctx.insert_bi_inst(BinaryOp::Sub, add_val, mul_val),
                }
            },
        }
    }
}

impl GenerateIR for MulExp {
    type RetType = Value;

    fn generate(&self, env: &mut Env) -> Self::RetType {
        match self {
            MulExp::Unary(unary_exp) => unary_exp.generate(env),
            MulExp::MulUnary(mul_exp, mul_op, unary_exp) => {
                let mul_val = mul_exp.generate(env);
                let unary_val = unary_exp.generate(env);
                match mul_op {
                    MulOp::Mul => env.ctx.insert_bi_inst(BinaryOp::Mul, mul_val, unary_val),
                    MulOp::Div => env.ctx.insert_bi_inst(BinaryOp::Div, mul_val, unary_val),
                    MulOp::Mod => env.ctx.insert_bi_inst(BinaryOp::Mod, mul_val, unary_val),
                }
            },
        }
    }
}

impl GenerateIR for UnaryExp {
    type RetType = Value;

    fn generate(&self, env: &mut Env) -> Self::RetType {
        match self {
            UnaryExp::PrimaryExp(primary_exp) => primary_exp.generate(env),
            UnaryExp::Unary(unary_op, unary_exp) => {
                let unary_val = unary_exp.generate(env);
                match unary_op {
                    UnaryOp::Plus => unary_val,
                    UnaryOp::Minus => {
                        let zero = env.ctx.create_int_inst(0);
                        env.ctx.insert_bi_inst(BinaryOp::Sub, zero, unary_val)
                    },
                    UnaryOp::Not => {
                        let zero = env.ctx.create_int_inst(0);
                        env.ctx.insert_bi_inst(BinaryOp::Eq, zero, unary_val)
                    }
                }
            },
            UnaryExp::FuncCall(func_call) => func_call.generate(env),
        }
    }
}

impl GenerateIR for PrimaryExp {
    type RetType = Value;

    fn generate(&self, env: &mut Env) -> Self::RetType {
        match self {
            PrimaryExp::Num(num) => {
                env.ctx.create_int_inst(num.clone())
            },
            PrimaryExp::Exp(exp) => {
                exp.generate(env)
            },
            PrimaryExp::LVal(lval) => {
                match lval {
                    LVal::Ident(ident) => {
                        let const_val = env.scope.is_const(&ident);
                        match const_val {
                            Some(num) => env.ctx.create_int_inst(num),
                            None => {
                                // currently, load var from symbol table everytime

                                let is_array = env.scope.is_array(ident);
                                match is_array {
                                    true => {
                                        let is_pointer = env.scope.lookup_is_pointer(ident).unwrap();
                                        let zero = env.ctx.create_int_inst(0);
                                        let mut addr = match is_pointer {
                                            true => env.load_var(ident),
                                            false => {
                                                let addr = env.scope.lookup_var_addr(ident).unwrap();
                                                env.get_elem_inst(addr, zero)
                                            }
                                        };
                                        addr
                                    }
                                    false => {
                                        env.load_var(&ident)
                                    }
                                }
                            }
                        }
                    },
                    LVal::Array(ident, dims) => {
                        let is_pointer = env.scope.lookup_is_pointer(ident).unwrap();
                        let array_size = env.scope.lookup_dim_size(ident).unwrap();
                        
                        match is_pointer {
                            true => {
                                let mut addr = env.load_var(ident);
                                let mut iter = dims.iter();
                                if let Some(dim) = iter.next() {
                                    let index = dim.generate(env);
                                    addr = env.get_ptr_inst(addr, index);
                                }

                                for dim in iter {
                                    let index = dim.generate(env);
                                    addr = env.get_elem_inst(addr, index);
                                }
                                
                                if array_size + 1 == dims.len() { // Value
                                    env.load_val_by_addr(addr)
                                }
                                else { //Pointer
                                    let zero = env.ctx.create_int_inst(0);
                                    addr = env.get_elem_inst(addr, zero);
                                    addr
                                }
                            },
                            false => {
                                let mut addr = env.scope.lookup_var_addr(ident).unwrap();
                                for dim in dims.iter() {
                                    let index = dim.generate(env);
                                    addr = env.get_elem_inst(addr, index);
                                }

                                if array_size == dims.len() {
                                    env.load_val_by_addr(addr)
                                }
                                else {
                                    let zero = env.ctx.create_int_inst(0);
                                    addr = env.get_elem_inst(addr, zero);
                                    addr
                                }
                            },
                        }
                    }
                }

            }
        }
    }
}


pub fn generate_random_string(length: usize) -> String {
    let rng = rand::thread_rng();
    String::from_utf8_lossy(&rng.sample_iter(&Alphanumeric).take(length).collect::<Vec<u8>>()).to_string()
}

