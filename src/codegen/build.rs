
use std::collections::HashMap;

use koopa::ir::{
    entities::{BasicBlock, ValueData},
    FunctionData, Program, Value, ValueKind,
};
use koopa::ir::types::TypeKind;
use koopa::ir::values::GlobalAlloc;


/// RISC-V 寄存器名称（下标对应寄存器编号），我们只用 t0/t1/t2/t6 临时计算，不做持久分配。
// 函数调用时，前 8 个参数放到 a0–a7（寄存器号 10–17），返回值放到 a0。
const REGISTER_NAMES: [&str; 32] = [
    "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "fp", "s1",
    "a0", "a1", "a2", "a3", "a4", "a5", "a6", "a7",
    "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11",
    "t3", "t4", "t5", "t6",
];
#[derive(Default)]
struct GlobalSymbolTable {
    symbols: HashMap<Value, String>, // Value -> String
    counter: usize, // 用于生成唯一的名字
}

impl GlobalSymbolTable {
    // 生成唯一的名字并存储
    fn generate_name(&mut self, value: Value) -> String {
        if let Some(name) = self.symbols.get(&value) {
            return name.clone();
        }

        let new_name = format!("global_{}", self.counter);
        self.symbols.insert(value, new_name.clone());
        self.counter += 1;
        new_name
    }
    pub fn get_global_name(&self, value: Value) -> Option<&str> {
        self.symbols.get(&value).map(|s| s.strip_prefix('@').unwrap_or(s.as_str()))
    }
}
/// 将整个 Koopa `program` 转成一整串 RISC-V 汇编字符串。
pub fn build_riscv(program: &Program) -> String {
    let mut output = String::new();
    let mut symbol_table = GlobalSymbolTable::default(); // 初始化符号表

    match program.build(program, &mut symbol_table) {
        Ok(lines) => {
            for line in lines {
                output.push_str(&line);
                output.push('\n');
            }
        }
        Err(e) => {
            eprintln!("Codegen error: {}", e);
            panic!("Code generation failed");
        }
    }
    output
}

/// 把各个 Koopa IR 组件编译成 RISC-V 指令行。
pub trait AssBuilder {
    fn build(&self, program: &Program, symbol_table: &mut GlobalSymbolTable) -> Result<Vec<String>, String>;
}


impl AssBuilder for Program {
    fn build(&self, _: &Program, symbol_table: &mut GlobalSymbolTable) -> Result<Vec<String>, String> {
        let mut program_codes = Vec::new();

        // 1) Emit 数据段（.data）
        program_codes.push(".data".to_string());
        for &global in self.inst_layout() {
            let vd = self.borrow_value(global);
            let global_name = symbol_table.generate_name(global);
            program_codes.push(format!("  .globl {}", global_name));
            program_codes.push(format!("{}:", global_name));
            program_codes.extend(vd.build(self, symbol_table)?);
        }

        // 2) Emit 代码段（.text）
        program_codes.push(".text".to_string());
        for &func in self.func_layout() {
            if self.func(func).layout().bbs().len() > 0 {
                program_codes.extend(self.func(func).build(self, symbol_table)?);
            }
        }
        Ok(program_codes)
    }
}

impl AssBuilder for FunctionData {
    fn build(&self, program: &Program, symbol_table: &mut GlobalSymbolTable) -> Result<Vec<String>, String> {
        let mut function_codes = Vec::new();

        // 函数名（去掉 leading '@'）
        let raw_name = self.name();
        let func_name = raw_name.strip_prefix('@').unwrap_or(raw_name);

        // --- 1) 为每个 BasicBlock 生成带编号的标签 ---
        let mut bb_labels: HashMap<BasicBlock, String> = HashMap::new();
        let mut bb_index: usize = 0;
        for (&bb, _) in self.layout().bbs() {
            let base_str: String = if let Some(bb_name) = &self
                .dfg()
                .bbs()
                .get(&bb)
                .expect("应该能找到 BasicBlock")
                .name()
            {
                let raw = bb_name.as_str();
                raw.strip_prefix('%').unwrap_or(raw).to_string()
            } else {
                let dbg = format!("{:?}", bb);
                dbg.trim_start_matches("BasicBlock(")
                    .trim_end_matches(')')
                    .to_string()
            };
            let lbl = format!("{}_{}_{}", func_name, base_str, bb_index);
            bb_labels.insert(bb, lbl);
            bb_index += 1;
        }

        // --- 2) 收集所有本地 Value 并分配栈槽 ---
        // 这里只把函数内部的“本地” Value（参数和基本块里出现的）注册到 slot_offsets
        let mut slot_offsets: HashMap<Value, i32> = HashMap::new();
        // fp+0 放旧的 fp，fp+4 放旧的 ra，fp+8 开始放本地 Value
        let mut current_offset: i32 = 8;

        // 2.1) 给函数参数分配栈槽
        for &param in self.params() {
            slot_offsets.insert(param, current_offset);
            current_offset += 4;
        }

        // 2.2) 提前把基本块里所有 insts() 中出现的本地 Value 都插一遍
        for (_, node) in self.layout().bbs() {
            for &value in node.insts().keys() {
                if slot_offsets.contains_key(&value) {
                    continue;
                }
                // 如果是 alloc（可能是数组或指针），一次性分配它的整个类型大小
                let value_data = self.dfg().value(value);
                let size_bytes = if let ValueKind::Alloc(_) = value_data.kind() {
                    // alloc 的类型有可能是数组或更复杂的结构，直接用 ty().size() 拿到字节数
                    value_data.ty().size() as i32
                } else {
                    // 其他普通值（整数常量、运算结果等），按 4 字节分配
                    4
                };
                slot_offsets.insert(value, current_offset);
                current_offset += size_bytes;
            }
        }

        // 向 4 字节对齐
        let total_slots = ((current_offset + 3) / 4) * 4;
        // 向 16 字节对齐
        let frame_size = ((total_slots + 15) / 16) * 16;

        // --- 3) Emit 函数 prologue ---
        function_codes.push(format!(".globl {}", func_name));
        function_codes.push(format!("{}:", func_name));

        // --- frame_size 可能超出 ±2047，要用 li+t6 再 sub sp ---
        if frame_size > 0 {
            if (-2048..=2047).contains(&(-frame_size)) {
                // frame_size 在范围内，直接 addi
                function_codes.push(format!("  addi\tsp, sp, -{}", frame_size));
            } else {
                // 超范围，将 frame_size 装到 t6 再 sub
                function_codes.push(format!("  li\t{}, {}", REGISTER_NAMES[31], frame_size)); // t6 = frame_size
                function_codes.push(format!("  sub\tsp, sp, {}", REGISTER_NAMES[31]));       // sp = sp - t6
            }
        }

        function_codes.push("  sw\tfp, 0(sp)".to_string());
        function_codes.push("  sw\tra, 4(sp)".to_string());
        function_codes.push("  addi\tfp, sp, 0".to_string());

        // 保存前 8 个参数到栈槽
        for (i, &param) in self.params().iter().enumerate().take(8) {
            let offset = *slot_offsets.get(&param).expect("参数必定已插入 slot_offsets");
            // --- 对偏移量做超范围检查 ---
            if (-2048..=2047).contains(&offset) {
                function_codes.push(format!(
                    "  sw\t{}, {}(fp)",
                    REGISTER_NAMES[10 + i], // a0–a7
                    offset
                ));
            } else {
                function_codes.push(format!("  li\t{}, {}", REGISTER_NAMES[31], offset)); // t6 = offset
                function_codes.push(format!(
                    "  add\t{}, fp, {}",
                    REGISTER_NAMES[31], REGISTER_NAMES[31]
                )); // t6 = fp + offset
                function_codes.push(format!(
                    "  sw\t{}, 0({})",
                    REGISTER_NAMES[10 + i], REGISTER_NAMES[31]
                )); // sw a_reg, 0(t6)
            }
        }

        // 保存第 9..nargs 个参数
        // 正确偏移要加上 frame_size
        let num_params = self.params().len();
        if num_params > 8 {
            for i in 8..num_params {
                let param = self.params()[i];
                let offset = *slot_offsets.get(&param).expect("参数必定已插入 slot_offsets");
                // 计算 caller 栈区里第 i 个参数所在偏移：frame_size + 4*(i-8)
                let caller_offset = frame_size + 4 * (i as i32 - 8);

                // --- 对 caller_offset 做范围检查 ---
                if (-2048..=2047).contains(&caller_offset) {
                    function_codes.push(format!(
                        "  lw\t{}, {}(fp)",
                        REGISTER_NAMES[5], // t0
                        caller_offset
                    ));
                } else {
                    function_codes.push(format!(
                        "  li\t{}, {}",
                        REGISTER_NAMES[31], caller_offset
                    )); // t6 = caller_offset
                    function_codes.push(format!(
                        "  add\t{}, fp, {}",
                        REGISTER_NAMES[31], REGISTER_NAMES[31]
                    )); // t6 = fp + caller_offset
                    function_codes.push(format!(
                        "  lw\t{}, 0({})",
                        REGISTER_NAMES[5], REGISTER_NAMES[31]
                    )); // lw t0, 0(t6)
                }

                // 再把 t0 存到本地的 slot
                if (-2048..=2047).contains(&offset) {
                    function_codes.push(format!("  sw\t{}, {}(fp)", REGISTER_NAMES[5], offset));
                } else {
                    function_codes.push(format!("  li\t{}, {}", REGISTER_NAMES[31], offset)); // t6 = offset
                    function_codes.push(format!(
                        "  add\t{}, fp, {}",
                        REGISTER_NAMES[31], REGISTER_NAMES[31]
                    )); // t6 = fp + offset
                    function_codes.push(format!(
                        "  sw\t{}, 0({})",
                        REGISTER_NAMES[5], REGISTER_NAMES[31]
                    )); // sw t0, 0(t6)
                }
            }
        }

        let mut saw_ret = false;

        // --- 4) 遍历基本块并生成指令 ---
        for (&bb, node) in self.layout().bbs() {
            if let Some(lbl) = bb_labels.get(&bb) {
                function_codes.push(format!("{}:", lbl));
            }

            for &value in node.insts().keys() {
                let value_data = self.dfg().value(value);
                match value_data.kind() {
                    // --- 整数常量: %dst = integer <imm> ---
                    ValueKind::Integer(int_val) => {
                        let offset = *slot_offsets.get(&value).unwrap_or_else(|| {
                            panic!("Value {:?} 没在 slot_offsets 注册", value)
                        });
                        function_codes.push(format!(
                            "  li\t{}, {}",
                            REGISTER_NAMES[5], // t0
                            int_val.value()
                        ));
                        // --- 对超范围偏移做处理 ---
                        if (-2048..=2047).contains(&offset) {
                            function_codes.push(format!(
                                "  sw\t{}, {}(fp)",
                                REGISTER_NAMES[5], offset
                            ));
                        } else {
                            function_codes.push(format!(
                                "  li\t{}, {}",
                                REGISTER_NAMES[31], offset
                            )); // t6 = offset
                            function_codes.push(format!(
                                "  add\t{}, fp, {}",
                                REGISTER_NAMES[31], REGISTER_NAMES[31]
                            )); // t6 = fp + offset
                            function_codes.push(format!(
                                "  sw\t{}, 0({})",
                                REGISTER_NAMES[5], REGISTER_NAMES[31]
                            )); // sw t0, 0(t6)
                        }
                    }

                    // --- 二元运算: %dst = binary %lhs, %rhs ---
                    ValueKind::Binary(binary) => {
                        let dst_offset = *slot_offsets.get(&value).unwrap_or_else(|| {
                            panic!("Value {:?} 没在 slot_offsets 注册", value)
                        });

                        // 1) 取 lhs
                        match self.dfg().value(binary.lhs()).kind() {
                            ValueKind::Integer(lhs_int) => {
                                function_codes.push(format!(
                                    "  li\t{}, {}",
                                    REGISTER_NAMES[5], // t0
                                    lhs_int.value()
                                ));
                            }
                            _ => {
                                let lhs_offset = *slot_offsets
                                    .get(&binary.lhs())
                                    .unwrap_or_else(|| {
                                        panic!(
                                            "Value {:?} 没在 slot_offsets 注册",
                                            binary.lhs()
                                        )
                                    });
                                // --- load 超范围偏移 ---
                                if (-2048..=2047).contains(&lhs_offset) {
                                    function_codes.push(format!(
                                        "  lw\t{}, {}(fp)",
                                        REGISTER_NAMES[5], lhs_offset
                                    ));
                                } else {
                                    function_codes.push(format!(
                                        "  li\t{}, {}",
                                        REGISTER_NAMES[31], lhs_offset
                                    )); // t6 = lhs_offset
                                    function_codes.push(format!(
                                        "  add\t{}, fp, {}",
                                        REGISTER_NAMES[31], REGISTER_NAMES[31]
                                    ));
                                    function_codes.push(format!(
                                        "  lw\t{}, 0({})",
                                        REGISTER_NAMES[5], REGISTER_NAMES[31]
                                    )); // lw t0, 0(t6)
                                }
                            }
                        }

                        // 2) 取 rhs
                        match self.dfg().value(binary.rhs()).kind() {
                            ValueKind::Integer(rhs_int) => {
                                function_codes.push(format!(
                                    "  li\t{}, {}",
                                    REGISTER_NAMES[6], // t1
                                    rhs_int.value()
                                ));
                            }
                            _ => {
                                let rhs_offset = *slot_offsets
                                    .get(&binary.rhs())
                                    .unwrap_or_else(|| {
                                        panic!(
                                            "Value {:?} 没在 slot_offsets 注册",
                                            binary.rhs()
                                        )
                                    });
                                // --- load 超范围偏移 ---
                                if (-2048..=2047).contains(&rhs_offset) {
                                    function_codes.push(format!(
                                        "  lw\t{}, {}(fp)",
                                        REGISTER_NAMES[6], rhs_offset
                                    ));
                                } else {
                                    function_codes.push(format!(
                                        "  li\t{}, {}",
                                        REGISTER_NAMES[31], rhs_offset
                                    )); // t6 = rhs_offset
                                    function_codes.push(format!(
                                        "  add\t{}, fp, {}",
                                        REGISTER_NAMES[31], REGISTER_NAMES[31]
                                    ));
                                    function_codes.push(format!(
                                        "  lw\t{}, 0({})",
                                        REGISTER_NAMES[6], REGISTER_NAMES[31]
                                    )); // lw t1, 0(t6)
                                }
                            }
                        }

                        // 3) 生成指令结果放到 t3
                        let instr = match binary.op() {
                            koopa::ir::BinaryOp::Add => "add",
                            koopa::ir::BinaryOp::Sub => "sub",
                            koopa::ir::BinaryOp::Mul => "mul",
                            koopa::ir::BinaryOp::Div => "div",
                            koopa::ir::BinaryOp::Mod => "rem",
                            koopa::ir::BinaryOp::Eq => {
                                function_codes.push(format!(
                                    "  xor\t{}, {}, {}",
                                    REGISTER_NAMES[7], // t3
                                    REGISTER_NAMES[5], // t0
                                    REGISTER_NAMES[6]  // t1
                                ));
                                function_codes.push(format!(
                                    "  seqz\t{}, {}",
                                    REGISTER_NAMES[7], // t3
                                    REGISTER_NAMES[7]  // t3
                                ));
                                // --- 存储超范围偏移 ---
                                if (-2048..=2047).contains(&dst_offset) {
                                    function_codes.push(format!(
                                        "  sw\t{}, {}(fp)",
                                        REGISTER_NAMES[7], dst_offset
                                    ));
                                } else {
                                    function_codes.push(format!(
                                        "  li\t{}, {}",
                                        REGISTER_NAMES[31], dst_offset
                                    )); // t6 = dst_offset
                                    function_codes.push(format!(
                                        "  add\t{}, fp, {}",
                                        REGISTER_NAMES[31], REGISTER_NAMES[31]
                                    ));
                                    function_codes.push(format!(
                                        "  sw\t{}, 0({})",
                                        REGISTER_NAMES[7], REGISTER_NAMES[31]
                                    ));
                                }
                                continue;
                            }
                            koopa::ir::BinaryOp::NotEq => {
                                function_codes.push(format!(
                                    "  xor\t{}, {}, {}",
                                    REGISTER_NAMES[7], // t3
                                    REGISTER_NAMES[5], // t0
                                    REGISTER_NAMES[6]  // t1
                                ));
                                function_codes.push(format!(
                                    "  snez\t{}, {}",
                                    REGISTER_NAMES[7], // t3
                                    REGISTER_NAMES[7]  // t3
                                ));
                                // --- 存储超范围偏移 ---
                                if (-2048..=2047).contains(&dst_offset) {
                                    function_codes.push(format!(
                                        "  sw\t{}, {}(fp)",
                                        REGISTER_NAMES[7], dst_offset
                                    ));
                                } else {
                                    function_codes.push(format!(
                                        "  li\t{}, {}",
                                        REGISTER_NAMES[31], dst_offset
                                    )); // t6 = dst_offset
                                    function_codes.push(format!(
                                        "  add\t{}, fp, {}",
                                        REGISTER_NAMES[31], REGISTER_NAMES[31]
                                    ));
                                    function_codes.push(format!(
                                        "  sw\t{}, 0({})",
                                        REGISTER_NAMES[7], REGISTER_NAMES[31]
                                    ));
                                }
                                continue;
                            }
                            koopa::ir::BinaryOp::Lt => "slt",
                            koopa::ir::BinaryOp::Gt => "sgt",
                            koopa::ir::BinaryOp::Le => {
                                function_codes.push(format!(
                                    "  sgt\t{}, {}, {}",
                                    REGISTER_NAMES[7], // t3
                                    REGISTER_NAMES[5], // t0
                                    REGISTER_NAMES[6]  // t1
                                ));
                                function_codes.push(format!(
                                    "  seqz\t{}, {}",
                                    REGISTER_NAMES[7], // t3
                                    REGISTER_NAMES[7]  // t3
                                ));
                                // --- 存储超范围偏移 ---
                                if (-2048..=2047).contains(&dst_offset) {
                                    function_codes.push(format!(
                                        "  sw\t{}, {}(fp)",
                                        REGISTER_NAMES[7], dst_offset
                                    ));
                                } else {
                                    function_codes.push(format!(
                                        "  li\t{}, {}",
                                        REGISTER_NAMES[31], dst_offset
                                    )); // t6 = dst_offset
                                    function_codes.push(format!(
                                        "  add\t{}, fp, {}",
                                        REGISTER_NAMES[31], REGISTER_NAMES[31]
                                    ));
                                    function_codes.push(format!(
                                        "  sw\t{}, 0({})",
                                        REGISTER_NAMES[7], REGISTER_NAMES[31]
                                    ));
                                }
                                continue;
                            }
                            koopa::ir::BinaryOp::Ge => {
                                function_codes.push(format!(
                                    "  slt\t{}, {}, {}",
                                    REGISTER_NAMES[7], // t3
                                    REGISTER_NAMES[5], // t0
                                    REGISTER_NAMES[6]  // t1
                                ));
                                function_codes.push(format!(
                                    "  seqz\t{}, {}",
                                    REGISTER_NAMES[7], // t3
                                    REGISTER_NAMES[7]  // t3
                                ));
                                // --- 存储超范围偏移 ---
                                if (-2048..=2047).contains(&dst_offset) {
                                    function_codes.push(format!(
                                        "  sw\t{}, {}(fp)",
                                        REGISTER_NAMES[7], dst_offset
                                    ));
                                } else {
                                    function_codes.push(format!(
                                        "  li\t{}, {}",
                                        REGISTER_NAMES[31], dst_offset
                                    )); // t6 = dst_offset
                                    function_codes.push(format!(
                                        "  add\t{}, fp, {}",
                                        REGISTER_NAMES[31], REGISTER_NAMES[31]
                                    ));
                                    function_codes.push(format!(
                                        "  sw\t{}, 0({})",
                                        REGISTER_NAMES[7], REGISTER_NAMES[31]
                                    ));
                                }
                                continue;
                            }
                            koopa::ir::BinaryOp::And => "and",
                            koopa::ir::BinaryOp::Or => "or",
                            koopa::ir::BinaryOp::Xor => "xor",
                            koopa::ir::BinaryOp::Shl => "sll",
                            koopa::ir::BinaryOp::Shr => "srl",
                            koopa::ir::BinaryOp::Sar => "sra",
                        };
                        function_codes.push(format!(
                            "  {}\t{}, {}, {}",
                            instr, REGISTER_NAMES[7], REGISTER_NAMES[5], REGISTER_NAMES[6]
                        ));
                        // 存储结果 t3 -> dst_offset(fp)
                        if (-2048..=2047).contains(&dst_offset) {
                            function_codes.push(format!(
                                "  sw\t{}, {}(fp)",
                                REGISTER_NAMES[7], dst_offset
                            ));
                        } else {
                            function_codes.push(format!(
                                "  li\t{}, {}",
                                REGISTER_NAMES[31], dst_offset
                            )); // t6 = dst_offset
                            function_codes.push(format!(
                                "  add\t{}, fp, {}",
                                REGISTER_NAMES[31], REGISTER_NAMES[31]
                            ));
                            function_codes.push(format!(
                                "  sw\t{}, 0({})",
                                REGISTER_NAMES[7], REGISTER_NAMES[31]
                            ));
                        }
                    }

                    // --- 局部分配: %slot = alloc i32/数组/指针 ---
                    ValueKind::Alloc(_) => {
                        // 对 alloc，不再把“指针”存到 slot；只要把 slot 自身当作数组的基址。
                        // 这样后续 getelemptr 直接用 fp + dst_offset 计算即可，无需把指针写回栈。
                        // 既不需要额外指令，也不能覆盖这个 slot 的内容。
                        // 所以这一处什么都不 emit，保留 slot_offsets 就行。
                    }

                    // --- 存储: store %value, %ptr ---
                    ValueKind::Store(st) => {
                        // 先打印标记说明这是 store
                        // 1) 先把要存的值加载到 t0
                        let val_repr = match self.dfg().value(st.value()).kind() {
                            ValueKind::Integer(iv) => {
                                function_codes.push(format!(
                                    "  li\t{}, {}",
                                    REGISTER_NAMES[5], // t0
                                    iv.value()
                                ));
                                REGISTER_NAMES[5]
                            }
                            _ => {
                                let arg_offset = *slot_offsets.get(&st.value()).unwrap_or_else(
                                    || panic!("Store 的值 {:?} 没在 slot_offsets 注册", st.value()),
                                );
                                // --- load 超范围偏移 ---
                                if (-2048..=2047).contains(&arg_offset) {
                                    function_codes.push(format!(
                                        "  lw\t{}, {}(fp)",
                                        REGISTER_NAMES[5], arg_offset
                                    ));
                                } else {
                                    function_codes.push(format!(
                                        "  li\t{}, {}",
                                        REGISTER_NAMES[31], arg_offset
                                    )); // t6 = arg_offset
                                    function_codes.push(format!(
                                        "  add\t{}, fp, {}",
                                        REGISTER_NAMES[31], REGISTER_NAMES[31]
                                    )); // t6 = fp + arg_offset
                                    function_codes.push(format!(
                                        "  lw\t{}, 0({})",
                                        REGISTER_NAMES[5], REGISTER_NAMES[31]
                                    ));
                                }
                                REGISTER_NAMES[5]
                            }
                        };

                        // 2) 根据 dest 是否在 slot_offsets 来决定局部还是全局
                        if slot_offsets.contains_key(&st.dest()) {
                            // 本地指针：先从栈上把这个“指针值”恢复到 t1，然后 sw 值到 0(t1)
                            let dest_offset = *slot_offsets
                                .get(&st.dest())
                                .unwrap_or_else(|| {
                                    panic!("Store 目标 {:?} 没在 slot_offsets 注册", st.dest())
                                });

                            // 如果 dest 是 alloc 返回的 value，本身 slot 就是数组第一元素的地址，
                            // “取指针”应为 fp + dest_offset
                            let base_ptr_is_alloc = matches!(
                                self.dfg().value(st.dest()).kind(),
                                ValueKind::Alloc(_)
                            );
                            if base_ptr_is_alloc {
                                // 需要判断 dest_offset 范围
                                if (-2048..=2047).contains(&dest_offset) {
                                    function_codes.push(format!(
                                        "  addi\t{}, fp, {}",
                                        REGISTER_NAMES[6], dest_offset
                                    )); // t1 = &arr[0]
                                } else {
                                    function_codes.push(format!(
                                        "  li\t{}, {}",
                                        REGISTER_NAMES[31], dest_offset
                                    )); // t6 = dest_offset
                                    function_codes.push(format!(
                                        "  add\t{}, fp, {}",
                                        REGISTER_NAMES[6], REGISTER_NAMES[31]
                                    )); // t1 = fp + t6
                                }
                            } else {
                                // 先 load 指针到 t1
                                if (-2048..=2047).contains(&dest_offset) {
                                    function_codes.push(format!(
                                        "  lw\t{}, {}(fp)",
                                        REGISTER_NAMES[6], dest_offset
                                    ));
                                } else {
                                    function_codes.push(format!(
                                        "  li\t{}, {}",
                                        REGISTER_NAMES[31], dest_offset
                                    )); // t6 = dest_offset
                                    function_codes.push(format!(
                                        "  add\t{}, fp, {}",
                                        REGISTER_NAMES[31], REGISTER_NAMES[31]
                                    ));
                                    function_codes.push(format!(
                                        "  lw\t{}, 0({})",
                                        REGISTER_NAMES[6], REGISTER_NAMES[31]
                                    ));
                                }
                            }
                            // 把 t0 (val_repr) 存到 0(t1)
                            function_codes.push(format!(
                                "  sw\t{}, 0({})",
                                val_repr, REGISTER_NAMES[6]
                            ));
                        } else {
                            // 全局：用 t1 作为地址寄存器，避免覆盖 t0
                            let addr_reg = REGISTER_NAMES[6]; // t1
                            let dest_name = symbol_table.get_global_name(st.dest())
            .unwrap_or_else(|| panic!("Store 目标 {:?} 未在符号表中注册", st.dest()));
                            function_codes.push(format!("  la\t{}, {}", addr_reg, dest_name));
                            function_codes.push(format!("  sw\t{}, 0({})", val_repr, addr_reg));
                        }
                    }

                    // --- 加载: %dst = load %ptr ---
                    ValueKind::Load(ld) => {
                        // 先打印标记说明这是 load
                        // 1) 判断 src 是否在 slot_offsets
                        if slot_offsets.contains_key(&ld.src()) {
                            // 本地指针：先从栈上或 alloc 计算得到指针，再从 0(指针) 读值
                            let src_offset = *slot_offsets.get(&ld.src()).unwrap_or_else(|| {
                                panic!("Load 源 {:?} 没在 slot_offsets 注册", ld.src())
                            });

                            // 如果 src 是 alloc 返回的 value，本身 slot 就是数组第一元素地址
                            let base_ptr_is_alloc = matches!(
                                self.dfg().value(ld.src()).kind(),
                                ValueKind::Alloc(_)
                            );
                            if base_ptr_is_alloc {
                                // 需判断 src_offset 范围
                                if (-2048..=2047).contains(&src_offset) {
                                    function_codes.push(format!(
                                        "  addi\t{}, fp, {}",
                                        REGISTER_NAMES[5], src_offset
                                    )); // t0 = &arr[0]
                                } else {
                                    function_codes.push(format!(
                                        "  li\t{}, {}",
                                        REGISTER_NAMES[31], src_offset
                                    )); // t6 = src_offset
                                    function_codes.push(format!(
                                        "  add\t{}, fp, {}",
                                        REGISTER_NAMES[5], REGISTER_NAMES[31]
                                    )); // t0 = fp + t6
                                }
                            } else {
                                // 否则先 load 指针到 t0
                                if (-2048..=2047).contains(&src_offset) {
                                    function_codes.push(format!(
                                        "  lw\t{}, {}(fp)",
                                        REGISTER_NAMES[5], src_offset
                                    ));
                                } else {
                                    function_codes.push(format!(
                                        "  li\t{}, {}",
                                        REGISTER_NAMES[31], src_offset
                                    )); // t6 = src_offset
                                    function_codes.push(format!(
                                        "  add\t{}, fp, {}",
                                        REGISTER_NAMES[31], REGISTER_NAMES[31]
                                    ));
                                    function_codes.push(format!(
                                        "  lw\t{}, 0({})",
                                        REGISTER_NAMES[5], REGISTER_NAMES[31]
                                    ));
                                }
                            }

                            // 从 0(t0) 读取到 t1
                            function_codes.push(format!(
                                "  lw\t{}, 0({})",
                                REGISTER_NAMES[6], REGISTER_NAMES[5]
                            ));
                            // 把 t1 存回 dst_offset(fp)
                            let dst_offset = *slot_offsets.get(&value).unwrap_or_else(|| {
                                panic!("Load 结果 {:?} 没在 slot_offsets 注册", value)
                            });
                            if (-2048..=2047).contains(&dst_offset) {
                                function_codes.push(format!(
                                    "  sw\t{}, {}(fp)",
                                    REGISTER_NAMES[6], dst_offset
                                ));
                            } else {
                                function_codes.push(format!(
                                    "  li\t{}, {}",
                                    REGISTER_NAMES[31], dst_offset
                                )); // t6 = dst_offset
                                function_codes.push(format!(
                                    "  add\t{}, fp, {}",
                                    REGISTER_NAMES[31], REGISTER_NAMES[31]
                                ));
                                function_codes.push(format!(
                                    "  sw\t{}, 0({})",
                                    REGISTER_NAMES[6], REGISTER_NAMES[31]
                                ));
                            }
                        } else {
                            // 全局：用 t0 取地址，用 t1 从内存里读值
                            let addr_reg = REGISTER_NAMES[5]; // t0
                            let val_reg = REGISTER_NAMES[6];  // t1
                            let src_name = symbol_table.get_global_name(ld.src())
            .unwrap_or_else(|| panic!("Load 源 {:?} 未在符号表中注册", ld.src()));
                            function_codes.push(format!("  la\t{}, {}", addr_reg, src_name));
                            function_codes.push(format!("  lw\t{}, 0({})", val_reg, addr_reg));
                            let dst_offset = *slot_offsets.get(&value).unwrap_or_else(|| {
                                panic!("Load 结果 {:?} 没在 slot_offsets 注册", value)
                            });
                            if (-2048..=2047).contains(&dst_offset) {
                                function_codes.push(format!(
                                    "  sw\t{}, {}(fp)",
                                    val_reg, dst_offset
                                ));
                            } else {
                                function_codes.push(format!(
                                    "  li\t{}, {}",
                                    REGISTER_NAMES[31], dst_offset
                                )); // t6 = dst_offset
                                function_codes.push(format!(
                                    "  add\t{}, fp, {}",
                                    REGISTER_NAMES[31], REGISTER_NAMES[31]
                                ));
                                function_codes.push(format!(
                                    "  sw\t{}, 0({})",
                                    val_reg, REGISTER_NAMES[31]
                                ));
                            }
                        }
                    }

                    // --- 分支: branch %cond, %then_bb, %else_bb ---
                    ValueKind::Branch(branch) => {
                        let cond_repr = match self.dfg().value(branch.cond()).kind() {
                            ValueKind::Integer(int_val) => {
                                function_codes.push(format!(
                                    "  li\t{}, {}",
                                    REGISTER_NAMES[5], // t0
                                    int_val.value()
                                ));
                                REGISTER_NAMES[5]
                            }
                            _ => {
                                let cond_offset =
                                    *slot_offsets.get(&branch.cond()).unwrap_or_else(|| {
                                        panic!(
                                            "Branch 条件 {:?} 没在 slot_offsets 注册",
                                            branch.cond()
                                        )
                                    });
                                // --- load 超范围偏移 ---
                                if (-2048..=2047).contains(&cond_offset) {
                                    function_codes.push(format!(
                                        "  lw\t{}, {}(fp)",
                                        REGISTER_NAMES[5], cond_offset
                                    ));
                                } else {
                                    function_codes.push(format!(
                                        "  li\t{}, {}",
                                        REGISTER_NAMES[31], cond_offset
                                    )); // t6 = cond_offset
                                    function_codes.push(format!(
                                        "  add\t{}, fp, {}",
                                        REGISTER_NAMES[31], REGISTER_NAMES[31]
                                    ));
                                    function_codes.push(format!(
                                        "  lw\t{}, 0({})",
                                        REGISTER_NAMES[5], REGISTER_NAMES[31]
                                    ));
                                }
                                REGISTER_NAMES[5]
                            }
                        };
                        let then_lbl = &bb_labels[&branch.true_bb()];
                        let else_lbl = &bb_labels[&branch.false_bb()];
                        function_codes.push(format!("  bnez\t{}, {}", cond_repr, then_lbl));
                        function_codes.push(format!("  j\t{}", else_lbl));
                    }

                    // --- 跳转: jump %target_bb ---
                    ValueKind::Jump(jump) => {
                        let target_lbl = &bb_labels[&jump.target()];
                        function_codes.push(format!("  j\t{}", target_lbl));
                    }

                    // --- 函数调用: %dst = call %func, args... ---
                    ValueKind::Call(callv) => {
                        // 1) 函数名
                        let func_entity = callv.callee();
                        let func_data = program.func(func_entity);
                        let raw_callee_name = func_data.name();
                        let callee_name =
                            raw_callee_name.strip_prefix('@').unwrap_or(raw_callee_name);

                        // 2) 参数数量 (nargs) 与超过 8 个的数量 (extra)
                        let nargs = callv.args().len();
                        let extra = if nargs > 8 { nargs - 8 } else { 0 };

                        // 3) 若参数超过 8 个，则为第 9..nargs 个参数在调用者栈上腾空间并依次存储
                        if extra > 0 {
                            function_codes.push(format!("  addi\tsp, sp, -{}", extra * 4));
                            for j in 8..nargs {
                                let arg = callv.args()[j];
                                let offset_sp = 4 * (j as i32 - 8);
                                match self.dfg().value(arg).kind() {
                                    ValueKind::Integer(iv) => {
                                        function_codes.push(format!(
                                            "  li\t{}, {}",
                                            REGISTER_NAMES[5], // t0
                                            iv.value()
                                        ));
                                    }
                                    _ => {
                                        let arg_offset = *slot_offsets.get(&arg).unwrap_or_else(
                                            || panic!("Call 参数 {:?} 没在 slot_offsets 注册", arg),
                                        );
                                        // --- load 超范围偏移 ---
                                        if (-2048..=2047).contains(&arg_offset) {
                                            function_codes.push(format!(
                                                "  lw\t{}, {}(fp)",
                                                REGISTER_NAMES[5], arg_offset
                                            ));
                                        } else {
                                            function_codes.push(format!(
                                                "  li\t{}, {}",
                                                REGISTER_NAMES[31], arg_offset
                                            )); // t6 = arg_offset
                                            function_codes.push(format!(
                                                "  add\t{}, fp, {}",
                                                REGISTER_NAMES[31], REGISTER_NAMES[31]
                                            ));
                                            function_codes.push(format!(
                                                "  lw\t{}, 0({})",
                                                REGISTER_NAMES[5], REGISTER_NAMES[31]
                                            ));
                                        }
                                    }
                                }
                                function_codes.push(format!(
                                    "  sw\t{}, {}(sp)",
                                    REGISTER_NAMES[5], offset_sp
                                ));
                            }
                        }

                        // 4) 前 8 个参数装入 a0..a7
                        for i in 0..std::cmp::min(nargs, 8) {
                            let arg = callv.args()[i];
                            let a_reg = 10 + i; // a0..a7 对应寄存器号 10..17
                            match self.dfg().value(arg).kind() {
                                ValueKind::Integer(iv) => {
                                    function_codes.push(format!(
                                        "  li\t{}, {}",
                                        REGISTER_NAMES[a_reg], iv.value()
                                    ));
                                }
                                _ => {
                                    let arg_offset = *slot_offsets.get(&arg).unwrap_or_else(
                                        || panic!("Call 参数 {:?} 没在 slot_offsets 注册", arg),
                                    );
                                    // --- load 超范围偏移 ---
                                    if (-2048..=2047).contains(&arg_offset) {
                                        function_codes.push(format!(
                                            "  lw\t{}, {}(fp)",
                                            REGISTER_NAMES[a_reg], arg_offset
                                        ));
                                    } else {
                                        function_codes.push(format!(
                                            "  li\t{}, {}",
                                            REGISTER_NAMES[31], arg_offset
                                        )); // t6 = arg_offset
                                        function_codes.push(format!(
                                            "  add\t{}, fp, {}",
                                            REGISTER_NAMES[31], REGISTER_NAMES[31]
                                        ));
                                        function_codes.push(format!(
                                            "  lw\t{}, 0({})",
                                            REGISTER_NAMES[a_reg], REGISTER_NAMES[31]
                                        ));
                                    }
                                }
                            }
                        }

                        // 5) 调用指令
                        function_codes.push(format!("  call\t{}", callee_name));

                        // 6) 恢复 sp（如果有额外参数）
                        if extra > 0 {
                            function_codes.push(format!("  addi\tsp, sp, {}", extra * 4));
                        }

                        // 7) 把 a0（返回值）存到 dst 的栈槽
                        let dst_offset = *slot_offsets.get(&value).unwrap_or_else(|| {
                            panic!("Call 返回值 {:?} 没在 slot_offsets 注册", value)
                        });
                        // --- 存储超范围偏移 ---
                        if (-2048..=2047).contains(&dst_offset) {
                            function_codes.push(format!(
                                "  sw\t{}, {}(fp)",
                                REGISTER_NAMES[10], // a0
                                dst_offset
                            ));
                        } else {
                            function_codes.push(format!(
                                "  li\t{}, {}",
                                REGISTER_NAMES[31], dst_offset
                            )); // t6 = dst_offset
                            function_codes.push(format!(
                                "  add\t{}, fp, {}",
                                REGISTER_NAMES[31], REGISTER_NAMES[31]
                            ));
                            function_codes.push(format!(
                                "  sw\t{}, 0({})",
                                REGISTER_NAMES[10], REGISTER_NAMES[31]
                            ));
                        }
                    }

                    // --- getelemptr: %dst = getelemptr %base, %index ---
                    ValueKind::GetElemPtr(gep) => {
                        // 先打印一个标记，说明是 getelemptr
                        let dst_offset = *slot_offsets.get(&value).unwrap_or_else(|| {
                            panic!("Value {:?} not registered in slot_offsets", value)
                        });

                        // 1) 取基址指针（如果 base 是 alloc，就直接用 fp+offset；否则从 栈 页加载 / 或者 全局）
                        let base_val = gep.src();
                        let base_addr_reg = REGISTER_NAMES[5]; // t0
                        //不应该直接在value上找base_val,可能是全局变量，不在value上
                        let base_is_alloc = if slot_offsets.contains_key(&base_val) {
                            matches!(
                                self.dfg().value(base_val).kind(),
                                ValueKind::Alloc(_)
                            )
                        } else {
                            false
                        };

                        if base_is_alloc {
                            // 直接计算：t0 = fp + slot_offsets[base_val]
                            let base_offset = *slot_offsets.get(&base_val).unwrap();
                            if (-2048..=2047).contains(&base_offset) {
                                function_codes.push(format!(
                                    "  addi\t{}, fp, {}",
                                    base_addr_reg, base_offset
                                )); // t0 = &array_base
                            } else {
                                function_codes.push(format!(
                                    "  li\t{}, {}",
                                    REGISTER_NAMES[31], base_offset
                                )); // t6 = base_offset
                                function_codes.push(format!(
                                    "  add\t{}, fp, {}",
                                    base_addr_reg, REGISTER_NAMES[31]
                                )); // t0 = fp + t6
                            }
                        } else if slot_offsets.contains_key(&base_val) {
                            // 本地变量指针：先 load 指针：t0 = [fp + base_offset]
                            let base_offset = *slot_offsets.get(&base_val).unwrap();
                            if (-2048..=2047).contains(&base_offset) {
                                function_codes.push(format!(
                                    "  lw\t{}, {}(fp)",
                                    base_addr_reg, base_offset
                                ));
                            } else {
                                function_codes.push(format!(
                                    "  li\t{}, {}",
                                    REGISTER_NAMES[31], base_offset
                                )); // t6 = base_offset
                                function_codes.push(format!(
                                    "  add\t{}, fp, {}",
                                    REGISTER_NAMES[31], REGISTER_NAMES[31]
                                ));
                                function_codes.push(format!(
                                    "  lw\t{}, 0({})",
                                    base_addr_reg, REGISTER_NAMES[31]
                                ));
                            }
                        } else {
                            // 全局变量指针：直接加载全局变量名到 t0
                            //为base_val去掉前缀@
                            //在终端打印base_val

                            let base_name = symbol_table.get_global_name(base_val)
            .unwrap_or_else(|| panic!("全局变量 {:?} 未在符号表中注册", base_val));
                            function_codes.push(format!("  la\t{}, {}", base_addr_reg, base_name));

                        }

                        // 2) 取索引值到 t1（常量或变量）
                        let index_reg = REGISTER_NAMES[6]; // t1
                        match self.dfg().value(gep.index()).kind() {
                            ValueKind::Integer(int_val) => {
                                // 常量索引：li t1, constant
                                function_codes.push(format!(
                                    "  li\t{}, {}",
                                    index_reg, int_val.value()
                                ));
                            }
                            _ => {
                                // 变量索引：从栈加载
                                let index_offset = *slot_offsets.get(&gep.index()).unwrap();
                                if (-2048..=2047).contains(&index_offset) {
                                    function_codes.push(format!(
                                        "  lw\t{}, {}(fp)",
                                        index_reg, index_offset
                                    ));
                                } else {
                                    function_codes.push(format!(
                                        "  li\t{}, {}",
                                        REGISTER_NAMES[31], index_offset
                                    ));
                                    function_codes.push(format!(
                                        "  add\t{}, fp, {}",
                                        REGISTER_NAMES[31], REGISTER_NAMES[31]
                                    ));
                                    function_codes.push(format!(
                                        "  lw\t{}, 0({})",
                                        index_reg, REGISTER_NAMES[31]
                                    ));
                                }
                            }
                        }

                        // 3) 计算偏移量 = index * sizeof(element)
                        //    先拿到 “指针指向的类型”：如果 base_val 是本地，则从 dfg()；否则从 program.borrow_value()
                        let ptr_type = if slot_offsets.contains_key(&base_val) {
                            let local_vd = self.dfg().value(base_val);
                            local_vd.ty().clone()
                        } else {
                            //在终端打印base_val

                            let global_vd = program.borrow_value(base_val);
                            global_vd.ty().clone()
                        };
                        let pointed_type = if let TypeKind::Pointer(inner) = ptr_type.kind() {
                            inner.clone()
                        } else {
                            panic!("getelemptr 源类型不是指针: {:?}", ptr_type);
                        };
                        let element_type = if let TypeKind::Array(elem, _) = pointed_type.kind() {
                            elem.clone()
                        } else {
                            panic!("getelemptr 基址类型不是数组: {:?}", pointed_type);
                        };
                        let element_size = element_type.size() as i32;
                        let mul_temp_reg = REGISTER_NAMES[7]; // t3
                        function_codes.push(format!("  li\t{}, {}", mul_temp_reg, element_size)); // t3 = element_size
                        function_codes.push(format!(
                            "  mul\t{}, {}, {}",
                            index_reg, index_reg, mul_temp_reg
                        )); // t1 = index * element_size

                        // 4) 计算最终地址：add t0, t0, t1
                        function_codes.push(format!(
                            "  add\t{}, {}, {}",
                            base_addr_reg, base_addr_reg, index_reg
                        )); // t0 = base_addr + (index * element_size)

                        // 5) 保存结果到栈槽
                        if (-2048..=2047).contains(&dst_offset) {
                            function_codes.push(format!(
                                "  sw\t{}, {}(fp)",
                                base_addr_reg, dst_offset
                            ));
                        } else {
                            function_codes.push(format!(
                                "  li\t{}, {}",
                                REGISTER_NAMES[31], dst_offset
                            )); // t6 = dst_offset
                            function_codes.push(format!(
                                "  add\t{}, fp, {}",
                                REGISTER_NAMES[31], REGISTER_NAMES[31]
                            ));
                            function_codes.push(format!(
                                "  sw\t{}, 0({})",
                                base_addr_reg, REGISTER_NAMES[31]
                            ));
                        }
                    }

                    // --- getptr: %dst = getptr %src, %index ---
                    ValueKind::GetPtr(gep) => {

                        let dst_offset = *slot_offsets.get(&value).unwrap_or_else(|| {
                            panic!("Value {:?} not registered in slot_offsets", value)
                        });

                        // 1) 取基址指针（如果 base 是 alloc，就直接用 fp+offset；否则从栈或全局加载）
                        let base_val = gep.src();
                        let base_addr_reg = REGISTER_NAMES[5]; // t0
                        let base_is_alloc = matches!(
                            self.dfg().value(base_val).kind(),
                            ValueKind::Alloc(_)
                        );
                        if base_is_alloc {
                            let base_offset = *slot_offsets.get(&base_val).unwrap();
                            if (-2048..=2047).contains(&base_offset) {
                                function_codes.push(format!(
                                    "  addi\t{}, fp, {}",
                                    base_addr_reg, base_offset
                                ));
                            } else {
                                function_codes.push(format!(
                                    "  li\t{}, {}",
                                    REGISTER_NAMES[31], base_offset
                                ));
                                function_codes.push(format!(
                                    "  add\t{}, fp, {}",
                                    base_addr_reg, REGISTER_NAMES[31]
                                ));
                            }
                        } else if slot_offsets.contains_key(&base_val) {
                            let base_offset = *slot_offsets.get(&base_val).unwrap();
                            if (-2048..=2047).contains(&base_offset) {
                                function_codes.push(format!(
                                    "  lw\t{}, {}(fp)",
                                    base_addr_reg, base_offset
                                ));
                            } else {
                                function_codes.push(format!(
                                    "  li\t{}, {}",
                                    REGISTER_NAMES[31], base_offset
                                ));
                                function_codes.push(format!(
                                    "  add\t{}, fp, {}",
                                    REGISTER_NAMES[31], REGISTER_NAMES[31]
                                ));
                                function_codes.push(format!(
                                    "  lw\t{}, 0({})",
                                    base_addr_reg, REGISTER_NAMES[31]
                                ));
                            }
                        } else {
                            let base_name = symbol_table.get_global_name(base_val)
            .unwrap_or_else(|| panic!("全局变量 {:?} 未在符号表中注册", base_val));
                            function_codes.push(format!("  la\t{}, {}", base_addr_reg, base_name));
                        }

                        // 2) 取索引值到 t1（常量或变量）
                        let index_reg = REGISTER_NAMES[6]; // t1
                        match self.dfg().value(gep.index()).kind() {
                            ValueKind::Integer(int_val) => {
                                function_codes.push(format!(
                                    "  li\t{}, {}",
                                    index_reg, int_val.value()
                                ));
                            }
                            _ => {
                                let index_offset = *slot_offsets.get(&gep.index()).unwrap();
                                if (-2048..=2047).contains(&index_offset) {
                                    function_codes.push(format!(
                                        "  lw\t{}, {}(fp)",
                                        index_reg, index_offset
                                    ));
                                } else {
                                    function_codes.push(format!(
                                        "  li\t{}, {}",
                                        REGISTER_NAMES[31], index_offset
                                    ));
                                    function_codes.push(format!(
                                        "  add\t{}, fp, {}",
                                        REGISTER_NAMES[31], REGISTER_NAMES[31]
                                    ));
                                    function_codes.push(format!(
                                        "  lw\t{}, 0({})",
                                        index_reg, REGISTER_NAMES[31]
                                    ));
                                }
                            }
                        }

                        // 3) 计算元素大小（element_size）：
                        //    如果 base_val 是本地，就用 self.dfg().value(local).ty()；否则从全局拿 program.borrow_value()
                        let ptr_type = if slot_offsets.contains_key(&base_val)
                            || matches!(self.dfg().value(base_val).kind(), ValueKind::Alloc(_))
                        {
                            let local_vd = self.dfg().value(base_val);
                            local_vd.ty().clone()
                        } else {
                            //z
                            let global_vd = program.borrow_value(base_val);
                            global_vd.ty().clone()
                        };
                        let element_type = if let TypeKind::Pointer(inner) = ptr_type.kind() {
                            inner.clone()
                        } else {
                            panic!("getptr 源类型不是指针: {:?}", ptr_type);
                        };
                        let element_size = element_type.size() as i32;

                        // 把 element_size 装到 t3
                        let mul_temp_reg = REGISTER_NAMES[7]; // t3
                        function_codes.push(format!(
                            "  li\t{}, {}",
                            mul_temp_reg, element_size
                        ));

                        // 4) 计算偏移量 = index * element_size
                        function_codes.push(format!(
                            "  mul\t{}, {}, {}",
                            index_reg, index_reg, mul_temp_reg
                        ));

                        // 5) 计算最终地址：add t0, t0, t1
                        function_codes.push(format!(
                            "  add\t{}, {}, {}",
                            base_addr_reg, base_addr_reg, index_reg
                        ));

                        // 6) 保存结果到 dst_offset(fp)
                        if (-2048..=2047).contains(&dst_offset) {
                            function_codes.push(format!(
                                "  sw\t{}, {}(fp)",
                                base_addr_reg, dst_offset
                            ));
                        } else {
                            function_codes.push(format!(
                                "  li\t{}, {}",
                                REGISTER_NAMES[31], dst_offset
                            ));
                            function_codes.push(format!(
                                "  add\t{}, fp, {}",
                                REGISTER_NAMES[31], REGISTER_NAMES[31]
                            ));
                            function_codes.push(format!(
                                "  sw\t{}, 0({})",
                                base_addr_reg, REGISTER_NAMES[31]
                            ));
                        }
                    }

                    // --- 返回: return %opt ---
                    ValueKind::Return(ret) => {

                        saw_ret = true;
                        if let Some(val) = ret.value() {
                            match self.dfg().value(val).kind() {
                                ValueKind::Integer(int_val) => {
                                    function_codes.push(format!(
                                        "  li\t{}, {}",
                                        REGISTER_NAMES[5], // t0
                                        int_val.value()
                                    ));
                                }
                                _ => {
                                    let val_offset = *slot_offsets.get(&val).unwrap_or_else(|| {
                                        panic!("Return 值 {:?} 没在 slot_offsets 注册", val)
                                    });
                                    // --- load 超范围偏移 ---
                                    if (-2048..=2047).contains(&val_offset) {
                                        function_codes.push(format!(
                                            "  lw\t{}, {}(fp)",
                                            REGISTER_NAMES[5], val_offset
                                        ));
                                    } else {
                                        function_codes.push(format!(
                                            "  li\t{}, {}",
                                            REGISTER_NAMES[31], val_offset
                                        ));
                                        function_codes.push(format!(
                                            "  add\t{}, fp, {}",
                                            REGISTER_NAMES[31], REGISTER_NAMES[31]
                                        ));
                                        function_codes.push(format!(
                                            "  lw\t{}, 0({})",
                                            REGISTER_NAMES[5], REGISTER_NAMES[31]
                                        ));
                                    }
                                }
                            }
                            function_codes.push(format!("  mv\ta0, {}", REGISTER_NAMES[5]));
                        }
                        function_codes.push("  lw\tra, 4(sp)".to_string());
                        function_codes.push("  lw\tfp, 0(sp)".to_string());
                        if frame_size > 0 {
                            // --- 恢复 sp 时可能超范围，要用 li + add ---
                            if (-2048..=2047).contains(&frame_size) {
                                function_codes.push(format!("  addi\tsp, sp, {}", frame_size));
                            } else {
                                function_codes.push(format!(
                                    "  li\t{}, {}",
                                    REGISTER_NAMES[31], frame_size
                                )); // t6 = frame_size
                                function_codes.push(format!(
                                    "  add\tsp, sp, {}",
                                    REGISTER_NAMES[31]
                                )); // sp = sp + t6
                            }
                        }
                        function_codes.push("  ret".to_string());
                    }

                    _ => {
                        return Err(format!(
                            "Unsupported instruction type: {:?}",
                            value_data.kind()
                        ));
                    }
                }
            }
        }

        // --- 5) 若从未遇到任何 Return，则补一个“默认 ret 0” ---
        if !saw_ret {
            function_codes.push("  li\ta0, 0".to_string());
            function_codes.push("  lw\tra, 4(sp)".to_string());
            function_codes.push("  lw\tfp, 0(sp)".to_string());
            if frame_size > 0 {
                // --- 恢复 sp 时可能超范围，要用 li + add ---
                if (-2048..=2047).contains(&frame_size) {
                    function_codes.push(format!("  addi\tsp, sp, {}", frame_size));
                } else {
                    function_codes.push(format!(
                        "  li\t{}, {}",
                        REGISTER_NAMES[31], frame_size
                    )); // t6 = frame_size
                    function_codes.push(format!(
                        "  add\tsp, sp, {}",
                        REGISTER_NAMES[31]
                    )); // sp = sp + t6
                }
            }
            function_codes.push("  ret".to_string());
        }

        Ok(function_codes)
    }
}

impl AssBuilder for ValueData {
    fn build(&self, program: &Program, symbol_table: &mut GlobalSymbolTable) -> Result<Vec<String>, String> {
        if let ValueKind::GlobalAlloc(global) = self.kind() {
            let mut value_codes = Vec::new();
            
            // 获取初始化值对应的 Value
            let init_value = global.init();
            
            // 生成全局变量名称

            // --- 处理数组类型全局变量 ---
            if let TypeKind::Array(_, _) = self.ty().kind() {
                let init_value_data = program.borrow_value(global.init());

                match init_value_data.kind() {
                    ValueKind::ZeroInit(_) => {
                        let size_bytes = init_value_data.ty().size();
                        value_codes.push(format!("  .zero {}", size_bytes));
                    }
                    ValueKind::Aggregate(agg) => {
                        for &field in agg.elems() {
                            let field_data = program.borrow_value(field);
                            if let ValueKind::Integer(int_val) = field_data.kind() {
                                value_codes.push(format!("  .word {}", int_val.value()));
                            } else {
                                panic!(
                                    "全局数组的 Aggregate 元素不是整型常量: {:?}",
                                    field_data.kind()
                                );
                            }
                        }
                    }
                    other => {
                        panic!(
                            "全局数组的初始化既不是 ZeroInit 也不是 Aggregate，而是 {:?}",
                            other
                        );
                    }
                }
                return Ok(value_codes);
            }

            // --- 处理标量/聚合型全局变量 ---
            let init_value_data = program.borrow_value(global.init());

            // 递归初始化函数
            fn emit_initializer(
                init_data: &ValueData,
                program: &Program,
                codes: &mut Vec<String>,
            ) {
                match init_data.kind() {
                    ValueKind::Integer(int_val) => {
                        codes.push(format!("  .word {}", int_val.value()));
                    }
                    ValueKind::ZeroInit(_) => {
                        let size_bytes = init_data.ty().size();
                        codes.push(format!("  .zero {}", size_bytes));
                    }
                    ValueKind::Aggregate(agg) => {
                        for &field in agg.elems() {
                            let field_data = program.borrow_value(field);
                            emit_initializer(&field_data, program, codes);
                        }
                    }
                    other => panic!("不支持的全局初始化类型: {:?}", other),
                }
            }

            match init_value_data.kind() {
                ValueKind::ZeroInit(_) => {
                    let size_bytes = init_value_data.ty().size();
                    value_codes.push(format!("  .zero {}", size_bytes));
                }
                ValueKind::Integer(int_val) => {
                    value_codes.push(format!("  .word {}", int_val.value()));
                }
                ValueKind::Aggregate(_) => {
                    emit_initializer(&init_value_data, program, &mut value_codes);
                }
                other => panic!(
                    "全局变量有不受支持的初始化类型: {:?}",
                    other
                ),
            }

            Ok(value_codes)
        } else {
            panic!("ValueData::build 只能在 GlobalAlloc 类型上调用");
        }
    }
}