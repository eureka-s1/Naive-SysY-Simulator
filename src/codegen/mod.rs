mod generate;
mod env;
mod asm;
mod instruction;
mod label;
mod table;
mod valuegen;
mod array;
mod build;

use koopa::ir::*;
use asm::{AsmProgram, AsmGlobal, AsmLocal, Section};
use instruction::Inst;
use label::Label;
use build::build_riscv;
use env::{Context, Env};
use generate::GenerateAsm;


use std::io::Write;
use std::fs::File;


pub fn build_asm(program: &Program) -> String {
    build_riscv(&program)
}


pub fn emit_asm(asm_program: String, output: String) {

    // println!("{:#?}", asm_program);
    let asm_str =  asm_program;
    println!("{}", asm_str);

    let mut file =  File::create(output).expect("Create file failed");
    file.write_all(asm_str.as_bytes()).expect("Write file failed");
}