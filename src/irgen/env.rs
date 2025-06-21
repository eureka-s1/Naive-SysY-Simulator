use koopa::ir::{builder::{BasicBlockBuilder, GlobalInstBuilder, LocalInstBuilder, ValueBuilder}, BasicBlock, BinaryOp, Function, FunctionData, Program, Type, TypeKind, Value, ValueKind};

use super::loopstack::LoopStack;
use super::scope::{Scope, Var, VarValue};
use super::array;

macro_rules! insert_inst_into_bb {
    ($func_data:expr, $ctx:expr, $inst:expr) => {
        let _ = $func_data
            .layout_mut()
            .bb_mut($ctx.block.expect("No block in context"))
            .insts_mut()
            .push_key_back($inst);
    };
}

// Representation of the path information currently being processed
#[derive(Default)]
pub struct Context {
    pub program: Program,
    pub func: Option<Function>,
    pub block: Option<BasicBlock>,
    pub block_count: usize,
}

// Context, Symbol Table, ...
#[derive(Default)]
pub struct Env {
    pub ctx: Context,
    pub scope: Scope,
    pub loopstack: LoopStack,
} 

impl Context {
    // create a integer value in current basic block
    pub fn create_int_inst(&mut self, num: i32) -> Value {
        let func = self.func.expect("No function in context");
        let func_data = self.program.func_mut(func);
    
        func_data.dfg_mut().new_value().integer(num)
    }

    // create a binary exp value in current basic block
    pub fn create_bi_inst(&mut self, op: BinaryOp, lhs: Value, rhs: Value) -> Value {
        let func = self.func.expect("No function in context");
        let func_data = self.program.func_mut(func);
    
        func_data.dfg_mut().new_value().binary(op, lhs, rhs)
    }

    // insert a integer instruction into current basic block
    pub fn insert_int_inst(&mut self, num: i32) -> Value {
        let func = self.func.expect("No function in context");
        let func_data = self.program.func_mut(func);
    
        let inst = func_data.dfg_mut().new_value().integer(num);
    
        let _ = func_data.
            layout_mut().
            bb_mut(self.block.expect("No block in context")).
            insts_mut().
            push_key_back(inst);

        inst 
    }

    // insert a binary instruction into current basic block
    pub fn insert_bi_inst(&mut self, op: BinaryOp, lhs: Value, rhs: Value) -> Value {
        let func = self.func.expect("No function in context");
        let func_data = self.program.func_mut(func);
    
        let inst = func_data.dfg_mut().new_value().binary(op, lhs, rhs);
    
        let _ = func_data.
            layout_mut().
            bb_mut(self.block.expect("No block in context")).
            insts_mut().
            push_key_back(inst);
            
        inst 
    }

    pub fn create_block(&mut self, bb_func: Option<String>) -> BasicBlock {
        let func = self.func.expect("No function in context");
        let func_data = self.program.func_mut(func);

        let func_id = func_data.name()[1..].to_string();
        let bb_id = match bb_func {
            Some(bb_func_s) => format!("%{}_{}{}", func_id, self.block_count.to_string(), bb_func_s),
            None => format!("{}_{}", func_id, self.block_count.to_string()),
        };

        let block = func_data.
            dfg_mut().
            new_bb().
            basic_block(Some(bb_id));

        func_data.layout_mut().bbs_mut().extend([block]);
        
        self.block_count += 1;
        block
    }

    pub fn remove_bb(&mut self, bb: BasicBlock) -> () {
        let func = self.func.expect("No function in context");
        let func_data = self.program.func_mut(func);
        func_data.layout_mut().bbs_mut().remove(&bb);
    }
}


impl Env {
    pub fn create_const_var(&mut self, ident: &String, num: i32) -> () {
        let var = Var::new_const(ident.clone(), num);
        self.scope.insert_var(var, VarValue::Const(num));
    }

    pub fn create_var(&mut self, ident: &String, val: Option<Value>) -> () {
        let var = Var::new_normal(ident.clone(), val);
        self.scope.insert_var(var, VarValue::Alloc(val, None, None));
    }

    pub fn alloc_var(&mut self, ident: &String) -> Value {
        // only i32
        let func = self.ctx.func.expect("No function in context");
        let func_data = self.ctx.program.func_mut(func);
    
        let inst = func_data.dfg_mut().new_value().alloc(Type::get_i32());

        let _ = func_data.
            layout_mut().
            bb_mut(self.ctx.block.expect("No block in context")).
            insts_mut().
            push_key_back(inst);
        
        // insert var into symbol table
        let var = Var::new_normal(ident.clone(), Some(inst));
        self.scope.insert_var(var, VarValue::Alloc(Some(inst), None, None));

        inst
    }

    // in funcdecl, actually allocate a pointer of an array
    pub fn func_alloc_pointer(&mut self, ident: &String, ty: Type, dims: &Vec<i32>) -> Value {
        // only i32
        let func = self.ctx
        .func.expect("No function in context");
        let func_data = self.ctx.program.func_mut(func);

        let inst = func_data.dfg_mut().new_value().alloc(ty);

        insert_inst_into_bb!(func_data, self.ctx, inst); // ToDo : subtitute

        let var = Var::new_array(ident.clone(), true, dims.clone(), Some(inst)); 
        self.scope.insert_var(var, VarValue::Alloc(Some(inst), Some(dims.clone()), Some(true)));

        inst        
    }

    pub fn alloc_global_var(&mut self, ident: &String, num: i32) -> Value {
        // only i32
        let num_val = self.ctx.program.new_value().integer(num);
        let inst = self.ctx.program.new_value().global_alloc(num_val);
        
        // insert var into symbol table
        let var = Var::new_normal(ident.clone(), Some(inst));
        self.scope.insert_var(var, VarValue::Alloc(Some(inst), None, None));
        
        inst
    }

    pub fn alloc_global_array(&mut self, ident: &String, dims: &Vec<i32>, init_val: Value) -> Value {
        // all ready
        let inst = self.ctx.program.new_value().global_alloc(init_val);
        
        let var = Var::new_array(ident.clone(), false, dims.clone(), Some(inst)); 
        self.scope.insert_var(var, VarValue::Alloc(Some(inst), Some(dims.clone()), Some(false)));

        inst
    }

    pub fn alloc_array(&mut self, ident: &String, dims: &Vec<i32>, init_val: Option<Vec<Value>>) {
        
        let func = self.ctx.func.expect("No function in context");
        let func_data = self.ctx.program.func_mut(func);
        let ty = array::get_array(&dims);
        let mut inst = func_data.dfg_mut().new_value().alloc(ty);

        let var = Var::new_array(ident.clone(), false, dims.clone(), Some(inst)); 
        self.scope.insert_var(var, VarValue::Alloc(Some(inst), Some(dims.clone()), Some(false)));
        
        insert_inst_into_bb!(func_data, self.ctx, inst);

        if let None = init_val {
            ()
        }
        else {
            // A series of store inst
            for _ in 0..dims.len() as i32 {
                let index = func_data.dfg_mut().new_value().integer(0);
                inst = func_data.dfg_mut().new_value().get_elem_ptr(
                    inst, 
                    index,
                );
                insert_inst_into_bb!(func_data, self.ctx, inst);
            }  

            // zip 
            for val in init_val.unwrap().iter().zip(0..) {
                let index = func_data.dfg_mut().new_value().integer(val.1);
                let pos = func_data.dfg_mut().new_value().get_ptr(
                    inst,
                    index
                );
                insert_inst_into_bb!(func_data, self.ctx, pos);
                let store = func_data.dfg_mut().new_value().store(*val.0, pos);
                insert_inst_into_bb!(func_data, self.ctx, store);
            }  
            
        }

    }

    // insert a load var into current basic block
    pub fn load_var(&mut self, ident: &String) -> Value {
        let var = self.scope.lookup_var(ident).expect("Var not found");
        match var {
            VarValue::Const(_) => panic!("Const var has been loaded in eval"),
            VarValue::Alloc(None, ..) => panic!("Var not allocated"),
            VarValue::Alloc(Some(addr),..) => {
                let func = self.ctx.func.expect("No function in context");
                let func_data = self.ctx.program.func_mut(func);
                let inst = func_data.dfg_mut().new_value().load(addr);

                let _ = func_data.
                    layout_mut().
                    bb_mut(self.ctx.block.expect("No block in context")).
                    insts_mut().
                    push_key_back(inst);
                
                inst
            }
            VarValue::Func(_) => panic!("Func cannot be loaded"),
        }
    }

    pub fn store_var(&mut self, ident: &String, val: Value) -> () {
        let var = self.scope.lookup_var(ident).expect("Var not found");
        match var {
            VarValue::Const(_) => panic!("Const var cannot be stored"),
            VarValue::Alloc(None,..) => panic!("Var not allocated"),
            VarValue::Alloc(Some(addr),..) => {
                let func = self.ctx.func.expect("No function in context");
                let func_data = self.ctx.program.func_mut(func);
                let inst = func_data.dfg_mut().new_value().store(val, addr);

                let _ = func_data.
                    layout_mut().
                    bb_mut(self.ctx.block.expect("No block in context")).
                    insts_mut().
                    push_key_back(inst);
            }
            VarValue::Func(_) => panic!("Func cannot be stored"),
        }
    }

    pub fn store_val_by_addr(&mut self, addr: Value, val: Value) -> () {
        let func = self.ctx.func.expect("No function in context");
        let func_data = self.ctx.program.func_mut(func);
        let inst = func_data.dfg_mut().new_value().store(val, addr);

        let _ = func_data.
            layout_mut().
            bb_mut(self.ctx.block.expect("No block in context")).
            insts_mut().
            push_key_back(inst);
    }

    pub fn load_val_by_addr(&mut self, addr: Value) -> Value {
        let func = self.ctx.func.expect("No function in context");
        let func_data = self.ctx.program.func_mut(func);
        let inst = func_data.dfg_mut().new_value().load(addr);

        let _ = func_data.
            layout_mut().
            bb_mut(self.ctx.block.expect("No block in context")).
            insts_mut().
            push_key_back(inst);
        inst 
    }

    pub fn ret_inst(&mut self, val: Value) -> () {
        let func = self.ctx.func.expect("No function in context");
        let func_data = self.ctx.program.func_mut(func);
        let inst = func_data.dfg_mut().new_value().ret(Some(val));

        let _ = func_data.
            layout_mut().
            bb_mut(self.ctx.block.expect("No block in context")).
            insts_mut().
            push_key_back(inst);
    }

    pub fn ret_void_inst(&mut self) -> () {
        let func = self.ctx.func.expect("No function in context");
        let func_data = self.ctx.program.func_mut(func);
        let inst = func_data.dfg_mut().new_value().ret(None);
        
        let _ = func_data.
            layout_mut().
            bb_mut(self.ctx.block.expect("No block in context")).
            insts_mut().
            push_key_back(inst);
    }

    pub fn jump_inst(&mut self, bb: BasicBlock) -> () {
        let func = self.ctx.func.expect("No function in context");
        let func_data = self.ctx.program.func_mut(func);
        let inst = func_data.dfg_mut().new_value().jump(bb);

        let _ = func_data.
            layout_mut().
            bb_mut(self.ctx.block.expect("No block in context")).
            insts_mut().
            push_key_back(inst);
    }

    pub fn branch_inst(&mut self, cond: Value, then_bb: BasicBlock, else_bb: BasicBlock) -> () {
        let func = self.ctx.func.expect("No function in context");
        let func_data = self.ctx.program.func_mut(func);
        let inst = func_data.dfg_mut().new_value().branch(cond, then_bb, else_bb);

        let _ = func_data.
            layout_mut().
            bb_mut(self.ctx.block.expect("No block in context")).
            insts_mut().
            push_key_back(inst);
    }

    pub fn call_inst(&mut self, callee: Function, args: Vec<Value>) -> Value {
        let func = self.ctx.func.expect("No function in context");
        let func_data = self.ctx.program.func_mut(func);
        let inst = func_data.dfg_mut().new_value().call(callee, args);

        let _ = func_data.
            layout_mut().
            bb_mut(self.ctx.block.expect("No block in context")).
            insts_mut().
            push_key_back(inst);

        inst 
    }

    // ptr: *[T, N] -> ptr + index * sizeof(T): *T
    pub fn get_elem_inst(&mut self, ptr: Value, idx: Value) -> Value {
        let func = self.ctx.func.expect("No function in context");
        let func_data = self.ctx.program.func_mut(func);
        let inst = func_data.dfg_mut().new_value().get_elem_ptr(ptr, idx);

        let _ = func_data.
            layout_mut().
            bb_mut(self.ctx.block.expect("No block in context")).
            insts_mut().
            push_key_back(inst);
        
        inst
    }

    // ptr: *T -> ptr + index * sizeof(T): *T
    pub fn get_ptr_inst(&mut self, ptr: Value, idx: Value) -> Value {
        let func = self.ctx.func.expect("No function in context");
        let func_data = self.ctx.program.func_mut(func);
        let inst = func_data.dfg_mut().new_value().get_ptr(ptr, idx);

        let _ = func_data.
            layout_mut().
            bb_mut(self.ctx.block.expect("No block in context")).
            insts_mut().
            push_key_back(inst);

        inst
    }
}