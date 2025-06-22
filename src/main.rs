mod codegen;
mod irgen;
mod simulator;

// use koopa::back::KoopaGenerator;
use lalrpop_util::lalrpop_mod;
use std::{env::args};
use std::fs::read_to_string;
use std::process::exit;

use irgen::{build_ir, emit_ir};
use codegen::{build_asm, emit_asm};
mod riscv_codegen;

lalrpop_mod! {
    #[allow(clippy::all)]
    sysy
}

fn main() {

    if let Err(err) = try_main() {
        eprintln!("{}", err);
        exit(-1);
    }
}

#[allow(unused)]
fn try_main() -> Result<(), Box<dyn std::error::Error>> {
    // parse arguments
    let mut args = args();
    args.next();
    let mode = args.next().unwrap();
    let input = args.next().unwrap();
    args.next();
    let output = args.next().unwrap();
    
    // add dir prefix
    let input = format!("{}/{}", "testcase/c", input); 
    // println!("input: {}, output: {}", input, output);

    // read input file
    let input = read_to_string(input)?;

    // generate AST
    let ast = sysy::CompUnitParser::new()
        .parse(&input)
        .expect("Parse error");

    // println!("{:#?}", ast);

    // generate IR
    let program = build_ir(ast).unwrap();

    match mode.as_str() {
        "-koopa" => {
            let output = format!("{}/{}/{}", "testcase", "koopa", output);
            emit_ir(program, output);
        }
        "-riscv" => {
            // generate ASM
            let mut asm_program = riscv_codegen::codegen_assmembly(&program);
            let output = format!("{}/{}/{}", "testcase", "riscv", output);
            emit_asm(asm_program, output);
        }
        "-sim" => {
            // let mut asm_program = build_asm(&program);
            // let output = format!("{}/{}/{}.s", "testcase", "riscv", output);
            // emit_asm(asm_program, output);
            simulator::pipe_exc();
        }
        _ => panic!("Unsupported Mode"),
    }
    Ok(())
}