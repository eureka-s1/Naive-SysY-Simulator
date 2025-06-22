mod utils;
mod elf;
mod cpu;
mod mem;
mod pipe;


use object::pe::IMAGE_DEBUG_TYPE_FIXUP;
use pipe::Pipeline;
use mem::Memory;
use std::io::{self, Write};

pub fn pipe_exc() {
    let mut mem = Memory::new();

    // mem.load_image("testcase/c/hello").unwrap();
    mem.load_image("testcase/bin/dummy.bin").unwrap();

    let mut cpu = Pipeline::new();
    cpu.init();
    
    pipe_exc_once(&mut cpu, &mut mem, true);
    // let mut running = true;
    // while running {
    //     cpu.step(&mut mem);
    // }
}

pub fn pipe_exc_once(cpu: &mut Pipeline, mem: &mut Memory, mut debug_mode: bool) {
    let mut running = true;
    while running {
        if !debug_mode {
            cpu.step(mem);
            continue;
        }

        print!("sim> ");
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let mut parts = input.trim().split_whitespace();
        
        match parts.next() {
            Some("c") => {
                debug_mode = false;
                println!("Continuing execution");
            },
            Some("q") => {
                debug_mode = false;
                running = false;
                println!("Exiting simulator");
            },
            Some("si") => {
                let n = match parts.next() {
                    Some(num_str) => match num_str.parse::<u32>() {
                        Ok(n) => n,
                        Err(_) => {
                            println!("Invalid number");
                            continue;
                        }
                    },
                    None => 1, // 默认执行1次
                };

                for _ in 0..n {
                    if !running { break; }
                    cpu.step(mem);
                }
            },
            Some("info") => {
                match parts.next() {
                    Some("r") => cpu.print_state(mem),
                    Some(_) => println!("Invalid info subcommand"),
                    None => println!("Missing subcommand for info"),
                }
            },
            Some("x") => {
                // 检查内存
                let n = match parts.next() {
                    Some(n_str) => match n_str.parse::<usize>() {
                        Ok(n) => n,
                        Err(_) => {
                            println!("Invalid number");
                            continue;
                        }
                    },
                    None => {
                        println!("Missing count for x command");
                        continue;
                    }
                };

                let addr_str = match parts.next() {
                    Some(s) => s,
                    None => {
                        println!("Missing address for x command");
                        continue;
                    }
                };

                let addr = match parse_hex_address(addr_str) {
                    Ok(addr) => addr,
                    Err(e) => {
                        println!("{}", e);
                        continue;
                    }
                };

                for i in 0..n {
                    let current_addr = addr + (i * 4) as u64;
                    let data = mem.mem_read(current_addr, 4);
                    println!("0x{:08x}: 0x{:x}", current_addr, data.expect("mem read error"));
                }
            },
            Some("help") => {
                print_help();
            },
            Some(cmd) => {
                println!("Unknown command '{}'. Type 'help' for a list of commands.", cmd);
            },
            None => continue,
        }
    }
}

fn parse_hex_address(s: &str) -> Result<u64, String> {
    if s.starts_with("0x") || s.starts_with("0X") {
        u64::from_str_radix(&s[2..], 16).map_err(|_| format!("Invalid hex address: {}", s))
    } else {
        u64::from_str_radix(s, 16).map_err(|_| format!("Invalid hex address: {}", s))
    }
}

fn print_help() {
    println!("Available commands:");
    println!("  c          - Continue execution");
    println!("  q          - Quit the simulator");
    println!("  si [N]     - Single step execution (N times, default 1)");
    println!("  info r     - Print register state");
    println!("  x N ADDR   - Examine memory at address ADDR, N words");
    println!("              (ADDR format: 0x1234 or 1234)");
    println!("  help       - Print this help information");
}