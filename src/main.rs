mod codegen;
mod irgen;
pub mod ast_df;

// use koopa::back::KoopaGenerator;
use lalrpop_util::lalrpop_mod;
use std::{env::args};
use std::fs::read_to_string;
use std::process::exit;

use irgen::{build_ir, emit_ir};
use codegen::{build_asm, emit_asm};

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
        "-koopa" => emit_ir(program, output),
        _ => {
            // generate ASM
            let mut asm_program = build_asm(&program);
            emit_asm(asm_program, output);
        }
    }
    Ok(())
}