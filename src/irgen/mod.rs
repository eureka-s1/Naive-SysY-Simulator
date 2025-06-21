// mod decl;
// mod exp;
// mod stmt;

use crate::ast_df::*;
mod generate;
mod env;
mod scope;
mod eval;
mod loopstack;
mod opt;
mod builtin;
mod array;

use koopa::ir::*;
use koopa::back::KoopaGenerator;
use generate::GenerateIR;

use std::io::Write;
use std::fs::File;

pub fn build_ir(ast: CompUnit) -> Option<Program> {
    let mut env = env::Env::default();
    let _ = ast.generate(&mut env);
    Some(env.ctx.program)
}

pub fn emit_ir(program: Program, output: String) {
    // convert to text form
    let mut gen = KoopaGenerator::new(Vec::new());
    gen.generate_on(&program).unwrap();
    
    let text_form_ir = std::str::from_utf8(&gen.writer()).unwrap().to_string();
    println!("{}", text_form_ir);

    let mut file =  File::create(output).expect("Create file failed");
    file.write_all(text_form_ir.as_bytes()).expect("Write file failed");
   
}
