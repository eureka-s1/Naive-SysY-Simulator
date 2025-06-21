use koopa::ir::*;

#[derive(Debug, Default)]
pub struct LoopStack {
    entry_stack: Vec<BasicBlock>,
    exit_stack: Vec<BasicBlock>,
}  

impl LoopStack {
    pub fn new() -> LoopStack {
        LoopStack {
            entry_stack: Vec::new(),
            exit_stack: Vec::new(),
        }
    }

    pub fn push(&mut self, entry: &BasicBlock, exit: &BasicBlock) {
        self.entry_stack.push(entry.clone());
        self.exit_stack.push(exit.clone());
    }

    pub fn top(&self) -> (BasicBlock, BasicBlock) {
        let entry = self.entry_stack.last().unwrap();
        let exit = self.exit_stack.last().unwrap();
        (*entry, *exit)
    }



    pub fn pop(&mut self) -> (BasicBlock, BasicBlock) {
        let entry = self.entry_stack.pop().unwrap();
        let exit = self.exit_stack.pop().unwrap();
        (entry, exit)
    }
}

