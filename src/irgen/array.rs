
use koopa::ir::builder::ValueBuilder;
use koopa::ir::*;
use super::ast_df::*;
use crate::irgen::generate::GenerateIR;

use super::env::Env;
use super::eval::EvalExp;

use std::cmp;

pub fn get_array_p(dims: &Vec<i32>) -> Type {
    let mut _type = Type::get_i32();
    for dim in dims.iter().rev() {
        _type = Type::get_array(_type, dim.clone().try_into().unwrap());
    }
    Type::get_pointer(_type)
}

pub fn get_array(dims: &Vec<i32>) -> Type {
    let mut _type = Type::get_i32();
    for dim in dims.iter().rev() {
        _type = Type::get_array(_type, dim.clone().try_into().unwrap());
    }
    _type
}

pub fn find_align(dims: &Vec<i32>, mut len: i32, limit: i32) -> i32 {
    let mut align = 0;
    for dim in dims.iter().rev() {
        if len % dim == 0 {
            align += 1;
            len /= dim;
        }
        else {
            break;
        }
    }
    cmp::min(align, limit - 1)
}

pub fn global_const_init(env: &mut Env, dims: &Vec<i32>, init_val: &ConstInitVal) -> Value {

    // return the product of dims
    let total_num = dims.iter().fold(1, |acc, dim| acc * dim);
    // record current level 

    let mut vec = Vec::new();
    let _ = global_const_cur(env, dims, init_val, 0, dims.len() as i32 + 1, &mut vec);

    assert_eq!(vec.len() as i32, total_num);

    for dim in dims.iter().rev() {
        let mut vec_temp = Vec::new();
        let mut vec_res = Vec::new();

        for i in 1..=vec.len() {
            vec_temp.push(vec[i-1]);
            if i as i32 % dim == 0 {
                let val = env.ctx.program.new_value().aggregate(vec_temp.clone());
                vec_res.push(val);
                vec_temp.clear();
            }
        }
        vec = vec_res;
    }
    assert_eq!(vec.len(), 1);

    vec[0]
}

// limit: only consider the last [limit] dimension 
// local_len: the length of current elements in current init_list
pub fn global_const_cur(env: &mut Env, dims: &Vec<i32>, init_val: &ConstInitVal, pre_len: i32, limit: i32, vec: &mut Vec<Value>) -> i32{
    // record current level
    let mut len = 0;
    match init_val {
        ConstInitVal::ConstExp(val) => {
            let num = val.eval(env);
            vec.push(env.ctx.program.new_value().integer(num));
            len += 1;
        }
        ConstInitVal::InitList(init_list) => {
            let align = find_align(dims, pre_len, limit);

            for init_val in init_list {
                len += global_const_cur(env, dims, init_val, len, align, vec);
            }

            let total_len = dims.iter().rev().take(align as usize).fold(1, |acc, dim| acc * dim);

            for _ in 0..(total_len - len) {
                vec.push(env.ctx.program.new_value().integer(0));
            }
            len = total_len
        }
    }
    len
}


pub fn global_var_init(env: &mut Env, dims: &Vec<i32>, init_val: &InitVal) -> Value {

    // return the product of dims
    let total_num = dims.iter().fold(1, |acc, dim| acc * dim);
    // record current level 

    let mut vec = Vec::new();
    let _ = global_var_cur(env, dims, init_val, 0, dims.len() as i32 + 1, &mut vec);

    assert_eq!(vec.len() as i32, total_num);

    for dim in dims.iter().rev() {
        let mut vec_temp = Vec::new();
        let mut vec_res = Vec::new();

        for i in 1..=vec.len() {
            vec_temp.push(vec[i-1]);
            if i as i32 % dim == 0 {
                let val = env.ctx.program.new_value().aggregate(vec_temp.clone());
                vec_res.push(val);
                vec_temp.clear();
            }
        }
        vec = vec_res;
    }
    assert_eq!(vec.len(), 1);

    vec[0]
}

// limit: only consider the last [limit] dimension 
// local_len: the length of current elements in current init_list
pub fn global_var_cur(env: &mut Env, dims: &Vec<i32>, init_val: &InitVal, pre_len: i32, limit: i32, vec: &mut Vec<Value>) -> i32{
    // record current level
    let mut len = 0;
    match init_val {
        InitVal::Exp(exp) => {
            let num = exp.eval(env);
            vec.push(env.ctx.program.new_value().integer(num));
            len = 1;
        }
        InitVal::InitList(init_list) => {
            let align = find_align(dims, pre_len, limit);

            for init_val in init_list {
                len += global_var_cur(env, dims, init_val, len, align, vec);
            }

            let total_len = dims.iter().rev().take(align as usize).fold(1, |acc, dim| acc * dim);

            for _ in 0..(total_len - len) {
                vec.push(env.ctx.program.new_value().integer(0));
            }
            len = total_len
        }
    }
    len
}



pub fn local_const_init(env: &mut Env, dims: &Vec<i32>, init_val: &ConstInitVal) -> Vec<Value> {

    // return the product of dims
    let total_num = dims.iter().fold(1, |acc, dim| acc * dim);
    // record current level 

    let mut vec = Vec::new();
    let _ = local_const_cur(env, dims, init_val, 0, dims.len() as i32 + 1, &mut vec);

    assert_eq!(vec.len() as i32, total_num);
    vec
}

// limit: only consider the last [limit] dimension 
// local_len: the length of current elements in current init_list
pub fn local_const_cur(env: &mut Env, dims: &Vec<i32>, init_val: &ConstInitVal, pre_len: i32, limit: i32, vec: &mut Vec<Value>) -> i32{
    // record current level
    let mut len = 0;
    match init_val {
        ConstInitVal::ConstExp(exp) => {
            let num = exp.eval(env);        
            
            let func = env.ctx.func.expect("No function in context");
            let func_data = env.ctx.program.func_mut(func);
            let inst = func_data.dfg_mut().new_value().integer(num);

            vec.push(inst);
            len += 1;
        }
        ConstInitVal::InitList(init_list) => {
            let align = find_align(dims, pre_len, limit);

            for init_val in init_list {
                len += local_const_cur(env, dims, init_val, len, align, vec);
            }

            let total_len = dims.iter().rev().take(align as usize).fold(1, |acc, dim| acc * dim);

            for _ in 0..(total_len - len) {

                let func = env.ctx.func.expect("No function in context");
                let func_data = env.ctx.program.func_mut(func);
                let inst = func_data.dfg_mut().new_value().integer(0);

                vec.push(inst);
            }
            len = total_len
        }
    }
    len
}


pub fn local_var_init(env: &mut Env, dims: &Vec<i32>, init_val: &InitVal) -> Vec<Value> {
    // return the product of dims
    let total_num = dims.iter().fold(1, |acc, dim| acc * dim);
    // record current level 

    let mut vec = Vec::new();
    let _ = local_var_cur(env, dims, init_val, 0, dims.len() as i32 + 1, &mut vec);

    assert_eq!(vec.len() as i32, total_num);
    vec
}

// limit: only consider the last [limit] dimension 
// local_len: the length of current elements in current init_list
pub fn local_var_cur(env: &mut Env, dims: &Vec<i32>, init_val: &InitVal, pre_len: i32, limit: i32, vec: &mut Vec<Value>) -> i32{
    // record current level
    let mut len = 0;
    match init_val {
        InitVal::Exp(exp) => {
            let init_num = exp.generate(env);
            vec.push(init_num);
            len += 1;
        }
        InitVal::InitList(init_list) => {
            let align = find_align(dims, pre_len, limit);

            for init_val in init_list {
                len += local_var_cur(env, dims, init_val, len, align, vec);
            }

            let total_len = dims.iter().rev().take(align as usize).fold(1, |acc, dim| acc * dim);

            for _ in 0..(total_len - len) {
                let func = env.ctx.func.expect("No function in context");
                let func_data = env.ctx.program.func_mut(func);
                let inst = func_data.dfg_mut().new_value().integer(0);

                vec.push(inst);
            }
            len = total_len
        }
    }
    len
}
