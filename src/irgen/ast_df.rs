#[derive(Debug)]
pub struct CompUnit {
    pub items: Vec<GlobalItem>,
}

#[derive(Debug)]
pub enum GlobalItem {
    Decl(Decl),
    FuncDef(FuncDef),
}

#[derive(Debug)]  
pub struct FuncDef {
    pub func_type: BType,
    pub ident: String,
    pub params: Vec<FuncFParam>,
    pub block: Block,
}

#[derive(Debug, Clone)]
pub enum BType {
    Void,
    Int,
}

#[derive(Debug)]
pub struct FuncFParam {
    pub ty: BType,
    pub id: String,
    pub dims: Option<Vec<ConstExp>>,
}

#[derive(Debug)]
pub struct FuncCall {
    pub id: String,
    pub args: Vec<Exp>,
}

#[derive(Debug)]
pub struct Block {
    pub items: Vec<BlockItem>,
}

#[derive(Debug)]
pub enum BlockItem {
    Decl(Decl),
    Stmt(Stmt),
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
pub enum Stmt {
    Empty,
    Assign(Assign),
    Exp(Exp),
    Block(Block),
    Return(Return),
    If(If),
    While(While),
    Break,
    Continue,
}

#[derive(Debug)]
pub struct While {
    pub cond: Exp,
    pub stmt: Box<Stmt>,
}

#[derive(Debug)]
pub struct If {
    pub cond: Exp,
    pub stmt: Box<Stmt>,
    pub else_stmt: Option<Box<Stmt>>,
}

#[derive(Debug)]
pub struct Assign {
    pub lval: LVal,
    pub exp: Exp,
}

#[derive(Debug)]
pub struct Return {
    pub exp: Option<Exp>,
}


/////////////////
// Declaration //
/////////////////

#[derive(Debug)]
pub enum Decl {
    Const(ConstDecl),
    Var(VarDecl),
}

#[derive(Debug)]
pub struct ConstDecl {
    pub is_global: bool,
    pub const_defs: Vec<ConstDef>,
}

#[derive(Debug)]
pub struct ConstDef {
    pub ident: String,
    pub init_val: ConstInitVal,
    pub dims: Option<Vec<ConstExp>>,
}

#[derive(Debug)]
pub enum ConstInitVal {
    ConstExp(ConstExp),
    InitList(Vec<ConstInitVal>),
}

#[derive(Debug)]
pub struct VarDecl {
    pub is_global: bool,
    pub defs: Vec<VarDef>,
}

#[derive(Debug)]
pub struct VarDef {
    pub ident: String,
    pub init_val: Option<InitVal>,
    pub dims: Option<Vec<ConstExp>>,
}

#[derive(Debug)]
pub enum InitVal {
    Exp(Exp),
    InitList(Vec<InitVal>),
}

////////////////
//    Lval    //
////////////////
 
#[derive(Debug)]
pub enum LVal {
    Ident(String),
    Array(String, Vec<Exp>),
}

////////////////
// Expression //
////////////////

#[derive(Debug)]
pub enum Exp {
    LOrExp(LOrExp),
}

#[derive(Debug)]
pub enum PrimaryExp {
    LVal(LVal),
    Exp(Box<Exp>),
    Num(i32),
}

#[derive(Debug)]
pub enum UnaryExp {
    PrimaryExp(PrimaryExp),
    Unary(UnaryOp, Box<UnaryExp>),
    FuncCall(FuncCall),
}

#[derive(Debug)]
pub enum MulExp {
    Unary(UnaryExp),
    MulUnary(Box<MulExp>, MulOp, UnaryExp),
}

#[derive(Debug)]
pub enum AddExp {
    Mul(MulExp),
    AddMul(Box<AddExp>, AddOp, MulExp),
}

#[derive(Debug)]
pub enum RelExp {
    Add(AddExp),
    RelAdd(Box<RelExp>, RelOp, AddExp),
}

#[derive(Debug)]
pub enum EqExp {
    Rel(RelExp),
    EqRel(Box<EqExp>, EqOp, RelExp),
}

#[derive(Debug)]
pub enum LAndExp {
    Eq(EqExp),
    LAndEq(Box<LAndExp>, EqExp),
}

#[derive(Debug)]
pub enum LOrExp {
    LAnd(LAndExp),
    LOrLAnd(Box<LOrExp>, LAndExp),
}

#[derive(Debug)]
pub struct ConstExp {
    pub exp: Exp,
}

#[derive(Debug)]
pub enum UnaryOp {
    Plus,
    Minus,
    Not,
}

#[derive(Debug)]
pub enum MulOp {
    Mul,
    Div,
    Mod,
}

#[derive(Debug)]
pub enum AddOp {
    Add,
    Sub,
}

#[derive(Debug)]
pub enum RelOp {
    Lt,
    Gt,
    Le,
    Ge,
}

#[derive(Debug)]
pub enum EqOp {
    Eq,
    Neq,
}