use eframe::egui;
use crate::simulator::{Pipeline, Memory, parse_hex_address};

pub struct GraphicalShell {
    prog: Pipeline,
    mem: Memory,
    running: bool,
    debug_mode: bool,
    step_count: String,
    mem_addr: String,
    mem_count: String,
    mem_view: Vec<(u64, u64)>,
    console_output: String,
}

impl Default for GraphicalShell {
    fn default() -> Self {
        Self {
            prog: Pipeline::new(),
            mem: Memory::new(),
            running: true,
            debug_mode: true,
            step_count: "1".to_string(),
            mem_addr: "0x1000".to_string(),
            mem_count: "10".to_string(),
            mem_view: Vec::new(),
            console_output: "Simulator ready. Type 'help' for commands.\n".to_string(),
        }
    }
}

impl GraphicalShell {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Default::default()
    }

    fn add_console_output(&mut self, text: &str) {
        self.console_output.push_str(text);
        self.console_output.push('\n');
    }

    fn execute_command(&mut self, command: &str) {
        self.add_console_output(&format!("sim> {}", command));
        
        let mut parts = command.trim().split_whitespace();
        match parts.next() {
            Some("c") => {
                self.debug_mode = false;
                self.add_console_output("Continuing execution");
            },
            Some("q") => {
                self.debug_mode = false;
                self.running = false;
                self.add_console_output("Exiting simulator");
            },
            Some("si") => {
                let n = match parts.next() {
                    Some(num_str) => match num_str.parse::<u32>() {
                        Ok(n) => n,
                        Err(_) => {
                            self.add_console_output("Invalid number");
                            return;
                        }
                    },
                    None => 1,
                };

                for _ in 0..n {
                    if !self.running { break; }
                    self.prog.step(&mut self.mem);
                    self.running = self.prog.cpu.running;
                }
                self.add_console_output("Step executed");
            },
            Some("info") => {
                match parts.next() {
                    Some("r") => {
                        let state = self.prog.print_state(&self.mem);
                        self.add_console_output(&state);
                    },
                    Some(_) => self.add_console_output("Invalid info subcommand"),
                    None => self.add_console_output("Missing subcommand for info"),
                }
            },
            Some("x") => {
                let n = match parts.next() {
                    Some(n_str) => match n_str.parse::<usize>() {
                        Ok(n) => n,
                        Err(_) => {
                            self.add_console_output("Invalid number");
                            return;
                        }
                    },
                    None => {
                        self.add_console_output("Missing count for x command");
                        return;
                    }
                };

                let addr_str = match parts.next() {
                    Some(s) => s,
                    None => {
                        self.add_console_output("Missing address for x command");
                        return;
                    }
                };

                match parse_hex_address(addr_str) {
                    Ok(addr) => {
                        self.mem_view.clear();
                        for i in 0..n {
                            let current_addr = addr + (i * 4) as u64;
                            if let Ok(data) = self.mem.mem_read(current_addr, 4) {
                                self.mem_view.push((current_addr, data));
                            }
                        }
                    },
                    Err(e) => self.add_console_output(&e),
                }
            },
            Some("help") => {
                self.add_console_output("Available commands:");
                self.add_console_output("  c          - Continue execution");
                self.add_console_output("  q          - Quit the simulator");
                self.add_console_output("  si [N]     - Single step execution (N times, default 1)");
                self.add_console_output("  info r     - Print register state");
                self.add_console_output("  x N ADDR   - Examine memory at address ADDR, N words");
                self.add_console_output("              (ADDR format: 0x1234 or 1234)");
                self.add_console_output("  help       - Print this help information");
            },
            Some(cmd) => {
                self.add_console_output(&format!("Unknown command '{}'. Type 'help' for a list of commands.", cmd));
            },
            None => {},
        }
    }
}

impl eframe::App for GraphicalShell {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 非调试模式下自动执行
        if !self.debug_mode && self.running {
            self.prog.step(&mut self.mem);
            self.running = self.prog.cpu.running;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // 顶部控制栏
            egui::TopBottomPanel::top("control_panel").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Run").clicked() {
                        self.debug_mode = false;
                        self.add_console_output("Continuing execution");
                    }
                    if ui.button("Pause").clicked() {
                        self.debug_mode = true;
                        self.add_console_output("Entering debug mode");
                    }
                    if ui.button("Step").clicked() {
                        self.execute_command("si");
                    }
                    
                    ui.label("Steps:");
                    ui.text_edit_singleline(&mut self.step_count);
                    if ui.button("Step N").clicked() {
                        self.execute_command(&format!("si {}", self.step_count));
                    }
                    
                    if ui.button("Reset").clicked() {
                        *self = Self::default();
                    }
                    
                    ui.separator();
                    
                    ui.label("Status:");
                    ui.label(if self.running {
                        "Running"
                    } else {
                        "Stopped"
                    });
                });
            });

            // 主内容区域
            ui.horizontal(|ui| {
                // 左侧面板：寄存器和状态
                egui::SidePanel::left("register_panel").show_inside(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.heading("CPU State");
                        if ui.button("Refresh State").clicked() {
                            self.execute_command("info r");
                        }
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.monospace(&self.console_output);
                        });
                    });
                });

                // 中间分隔线
                ui.separator();

                // 右侧面板：内存查看器
                egui::SidePanel::right("memory_panel").show_inside(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.heading("Memory Viewer");
                        
                        ui.horizontal(|ui| {
                            ui.label("Address:");
                            ui.text_edit_singleline(&mut self.mem_addr);
                            
                            ui.label("Count:");
                            ui.text_edit_singleline(&mut self.mem_count);
                            
                            if ui.button("Examine").clicked() {
                                self.execute_command(&format!("x {} {}", self.mem_count, self.mem_addr));
                            }
                        });
                        
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            egui::Grid::new("memory_grid")
                                .num_columns(2)
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.label("Address");
                                    ui.label("Value");
                                    ui.end_row();
                                    
                                    for (addr, value) in &self.mem_view {
                                        ui.label(format!("0x{:08x}", addr));
                                        ui.label(format!("0x{:08x}", value));
                                        ui.end_row();
                                    }
                                });
                        });
                    });
                });
            });

            // 底部命令行
            egui::TopBottomPanel::bottom("command_panel").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let command_input = ui.text_edit_singleline(&mut String::new());
                    if ui.button("Execute").clicked() || command_input.lost_focus() {
                        if let Some(cmd) = command_input.text() {
                            if !cmd.trim().is_empty() {
                                self.execute_command(cmd);
                                command_input.clear();
                            }
                        }
                    }
                });
            });
        });

        // 请求重绘以保持动画
        ctx.request_repaint();
    }
}