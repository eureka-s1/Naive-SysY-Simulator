use eframe::{egui};
use super::pipe::*;
use super::mem::*;
use std::process;
pub struct GuiApp {
    pipeline: Pipeline,
    mem: Memory,
    step_counter: u32,
    debug_mode: bool,
    command_input: String,
    output: String,
    register_display: String,
    last_registers: [u64; 32], // 用于跟踪寄存器变化
}

impl Default for GuiApp {
    fn default() -> Self {
        let mut mem = Memory::new();
        mem.load_image("testcase/bin/load-store.bin").unwrap();
        let mut pipeline = Pipeline::new();
        pipeline.init(); 
        let last_registers = pipeline.cpu.reg.clone(); // 初始寄存器状态
        
        let mut app = Self {
            pipeline,
            mem,
            step_counter: 0,
            debug_mode: true,
            command_input: String::new(),
            output: String::new(),
            register_display: String::new(),
            last_registers, // 保存初始状态
        };
        
        app.update_register_display();
        app
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 请求持续重绘以确保UI更新
        ctx.request_repaint();
        
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Pipeline Simulator");
            
            // 使用两列布局
            ui.columns(2, |columns| {
                // 左列：寄存器状态
                columns[0].group(|ui| {
                    ui.label("Register State");
                    ui.add(
                        egui::TextEdit::multiline(&mut self.register_display)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                    );
                    ui.group(|ui| {
                        ui.label("CPU State");
                        ui.horizontal(|ui| {
                            ui.label("PC:");
                            ui.monospace(format!("0x{:016x}", self.pipeline.cpu.pc));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Cycle:");
                            ui.label(format!("{}", self.pipeline.cpu.cycle_count));
                        });
                        ui.horizontal(|ui| {
                            ui.label("Inst Count:");
                            ui.label(format!("{}", self.pipeline.cpu.inst_count));
                        });
                    });
                    
                    // 流水线阶段
                    ui.group(|ui| {
                        ui.label("Pipeline Stages");
                        ui.horizontal(|ui| {
                            ui.label("IF/ID:");
                            ui.monospace(format!("PC=0x{:08x}, INST=0x{:08x}", 
                                self.pipeline.D_reg.pc, self.pipeline.D_reg.inst));
                        });
                        ui.horizontal(|ui| {
                            ui.label("ID/EX:");
                            ui.monospace(format!("PC=0x{:08x}, RD={}", 
                                self.pipeline.E_reg.pc, self.pipeline.E_reg.rd));
                        });
                        ui.horizontal(|ui| {
                            ui.label("EX/MEM:");
                            ui.monospace(format!("PC=0x{:08x}, RD={}", 
                                self.pipeline.M_reg.pc, self.pipeline.M_reg.rd));
                        });
                        ui.horizontal(|ui| {
                            ui.label("MEM/WB:");
                            ui.monospace(format!("PC=0x{:08x}, RD={}", 
                                self.pipeline.W_reg.pc, self.pipeline.W_reg.rd));
                        });
                    });
                });

                // 右列：CPU状态、流水线阶段和其他内容
                columns[1].vertical(|ui| {
                    // CPU状态
                    
                    
                    // 命令输出
                    ui.group(|ui| {
                        ui.label("Output:");
                        ui.add(
                            egui::TextEdit::multiline(&mut self.output)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                        );
                    });

                    // 命令输入和执行按钮
                    ui.vertical(|ui| {
                        ui.label("Enter command:");
                        let response = ui.text_edit_singleline(&mut self.command_input);
                        if ui.button("Execute").clicked() || response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            self.execute_command();
                        }
                    });

                    ui.label(format!("Steps taken: {}", self.step_counter));
                });
            });
        });
    }
}

impl GuiApp {
    fn execute_command(&mut self,) {
        self.output.clear();
        let input = self.command_input.trim().to_lowercase();
        let mut parts = input.split_whitespace();
        
        match parts.next() {
            Some("c") => {
                self.debug_mode = false;
                self.output.push_str("Continuing execution\n");
                
                for _ in 0..100000 {
                    if !self.pipeline.cpu.running { break; }
                    let prev_pc = self.pipeline.cpu.pc;
                    self.pipeline.step(&mut self.mem);
                    self.step_counter += 1;
                    
                }
            },
            Some("q") => {
                self.debug_mode = false;
                self.pipeline.cpu.running = false;
                self.output.push_str("Exiting simulator\n");
                process::exit(0);
            },
            Some("si") => {
                let n = match parts.next() {
                    Some(num_str) => num_str.parse::<u32>().unwrap_or(1),
                    None => 1,
                };

                for _ in 0..n {
                    if !self.pipeline.cpu.running { break; }
                    
                    let prev_pc = self.pipeline.cpu.pc;
                    self.pipeline.step(&mut self.mem);
                    self.step_counter += 1;
                    
                }
                self.output.push_str(&format!("Executed {} steps\n", n));
            },
            Some("info") => {
                match parts.next() {
                    Some("r") => {
                        self.output.push_str("Register state:\n");
                        for i in 0..32 {
                            self.output.push_str(&format!("x{:02}: 0x{:016x}\n", i, self.pipeline.cpu.reg[i]));
                        }
                    },
                    Some(_) => self.output.push_str("Invalid info subcommand\n"),
                    None => self.output.push_str("Missing subcommand for info\n"),
                }
            },
            Some("x") => {
                
            },
            Some("help") => {
                self.print_help();
            },
            Some(cmd) => {
                self.output.push_str(&format!("Unknown command '{}'. Type 'help' for a list of commands.\n", cmd));
            },
            None => {}
        }

        // 检测寄存器变化并更新显示
        self.detect_register_changes();
        self.update_register_display();
        self.command_input.clear();
    }

    // 检测哪些寄存器发生了变化
    fn detect_register_changes(&mut self) {
        for i in 0..32 {
            if self.pipeline.cpu.reg[i] != self.last_registers[i] {
                self.output.push_str(&format!("Register x{} changed: 0x{:x} -> 0x{:x}\n", 
                    i, self.last_registers[i], self.pipeline.cpu.reg[i]));
                self.last_registers[i] = self.pipeline.cpu.reg[i];
            }
        }
    }

    // 更新寄存器显示的方法
    fn update_register_display(&mut self) {
        self.register_display.clear();
        
        // 添加通用寄存器
        self.register_display.push_str("General Registers:\n");
        for i in 0..32 {
            let value = self.pipeline.cpu.reg[i];
            let changed = if value != self.last_registers[i] { "*" } else { " " };
            self.register_display.push_str(&format!("x{:02}{}: 0x{:016x}\n", i, changed, value));
        }
    
    }

    fn print_help(&mut self) {
        self.output.push_str("Available commands:\n");
        self.output.push_str("  c          - Continue execution\n");
        self.output.push_str("  q          - Quit the simulator\n");
        self.output.push_str("  si [N]     - Single step execution (N times, default 1)\n");
        self.output.push_str("  info r     - Print register state to output\n");
        self.output.push_str("  x N ADDR   - Examine memory at address ADDR, N words\n");
        self.output.push_str("              (ADDR format: 0x1234 or 1234)\n");
        self.output.push_str("  help       - Print this help information\n");
    }
}


fn parse_hex_address(s: &str) -> Result<u64, String> {
    let s = s.trim_start_matches("0x").trim_start_matches("0X");
    u64::from_str_radix(s, 16).map_err(|_| format!("Invalid hex address: {}", s))
}

// 运行 GUI
pub fn run_gui() -> Result<(), eframe::Error> {
    eframe::run_native(
        "Pipeline Simulator",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1000.0, 800.0]),  // 增大窗口大小以容纳更多内容
            ..Default::default()
        },
        Box::new(|_cc| Box::<GuiApp>::default()),
    )
}