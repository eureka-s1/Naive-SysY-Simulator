use koopa::ir::{FunctionData, Type};

use super::env::Env;


pub fn builtin_decl(env: &mut Env) {
    
    let mut func = env.ctx.program.new_func(FunctionData::new_decl(
        "@getint".into(),
        vec![],
        Type::get_i32(),
    ));
    env.scope.insert_func(&"getint".into(), func);

    func = env.ctx.program.new_func(FunctionData::new_decl(
        "@getch".into(),
        vec![],
        Type::get_i32(),
    ));
    env.scope.insert_func(&"getch".into(), func);

    func = env.ctx.program.new_func(FunctionData::new_decl(
        "@getarray".into(),
        vec![Type::get_pointer(Type::get_i32())],
        Type::get_i32(),
    ));
    env.scope.insert_func(&"getarray".into(), func);

    func = env.ctx.program.new_func(FunctionData::new_decl(
        "@putint".into(),
        vec![Type::get_i32()],
        Type::get_i32(),
    ));
    env.scope.insert_func(&"putint".into(), func);

    func = env.ctx.program.new_func(FunctionData::new_decl(
        "@putch".into(),
        vec![Type::get_i32()],
        Type::get_unit(),
    ));
    env.scope.insert_func(&"putch".into(), func);

    func = env.ctx.program.new_func(FunctionData::new_decl(
        "@putarray".into(),
        vec![Type::get_i32(), Type::get_pointer(Type::get_i32())],
        Type::get_unit(),
    ));
    env.scope.insert_func(&"putarray".into(), func);

    func = env.ctx.program.new_func(FunctionData::new_decl(
        "@starttime".into(),
        vec![],
        Type::get_unit(),
    ));
    env.scope.insert_func(&"starttime".into(), func);

    func = env.ctx.program.new_func(FunctionData::new_decl(
        "@stoptime".into(),
        vec![],
        Type::get_unit(),
    ));
    env.scope.insert_func(&"stoptime".into(), func);
}