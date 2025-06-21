
// initvalue : riscv form
#[derive(Debug, Clone)]
pub enum InitVal {
    Word(i32),
    Zero(usize),
    Array(Vec<InitVal>),
}