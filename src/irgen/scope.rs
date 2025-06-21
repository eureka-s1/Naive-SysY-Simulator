use std::collections::{HashMap, HashSet};
use std::cell::RefCell;
use std::rc::Rc;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};

use koopa::ir::*;


type Ident = String;

// A stack of scopes
#[derive(Debug, Default)]
pub struct Scope {
    scopes : Vec<ScopeItem>, 
}

// A scope (for serveral basic blocks)
#[derive(Debug)]
struct ScopeItem {
    table : SymbolTable,
}


#[derive(Debug)]
struct SymbolTable {
    symbols : HashSet<Var>,
    vars : Rc<RefCell<HashMap<Ident, VarValue>>>,
}

impl Scope {
    pub fn enter_scope(&mut self) {
        self.scopes.push(ScopeItem {
            table : SymbolTable {
                symbols : HashSet::new(),
                vars : Rc::new(RefCell::new(HashMap::new())),
            },
        });
    }

    pub fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn insert_var(&mut self, var: Var, val : VarValue) {
        let scope = self.scopes.last_mut().unwrap();
        scope.table.symbols.insert(var.clone());
        scope.table.vars.borrow_mut().insert(var.ident(), val);
    }

    pub fn insert_func(&mut self, ident: &String, func: Function) {
        let scope = self.scopes.last_mut().unwrap();
        scope.table.symbols.insert(Var::Func(FuncVar{ident: ident.clone(), value: Some(func)}));
        scope.table.vars.borrow_mut().insert(ident.clone(), VarValue::Func(Some(func)));
    }

    // lookup for func
    pub fn lookup_func(&self, ident: &Ident) -> Option<Function> {
        for scope in self.scopes.iter() {
            if let Some(val) = scope.table.vars.borrow().get(ident) {
                return match val {
                    VarValue::Const(_) => None,
                    VarValue::Alloc(..) => None,
                    VarValue::Func(func) => func.clone(),
                };
            }
        }
        None
    }

    pub fn lookup_dim_size(&self, ident: &Ident) -> Option<usize> {
        for scope in self.scopes.iter().rev() {
            if let Some(var) = scope.table.vars.borrow().get(ident) {
                if let VarValue::Alloc(_, Some(dims), _) = var {
                    return Some(dims.len());
                }
            }
        }
        None
    }

    pub fn lookup_is_pointer(&self, ident: &Ident) -> Option<bool> {
        for scope in self.scopes.iter().rev() {
            if let Some(var) = scope.table.vars.borrow().get(ident) {
                if let VarValue::Alloc(_, _, is_pointer) = var {
                    return Some(is_pointer.unwrap());
                }
            }
        }
        None
    }

    pub fn is_array(&self, ident: &Ident) -> bool {
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.table.vars.borrow().get(ident) {
                return match val {
                    VarValue::Alloc(_, Some(..), _) => true,
                    _ => false,
                };
            }
        }
        false
    }

    // lookup for var (not function)
    pub fn lookup_var(&self, ident: &Ident) -> Option<VarValue> {
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.table.vars.borrow().get(ident) {
                return Some(val.clone());
            }
        }
        None
    }

    pub fn lookup_var_addr(&self, ident: &Ident) -> Option<Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.table.vars.borrow().get(ident) {
                return match val {
                    VarValue::Const(_) => panic!("Variable {} is a const var", ident),
                    VarValue::Alloc(val,..) => val.clone(),
                    VarValue::Func(_) => panic!("Variable {} is a function", ident),
                };
            }
        }
        panic!("Variable {} not found in this scope", ident)
    }


    pub fn is_const(&self, ident: &Ident) -> Option<i32> {
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.table.vars.borrow().get(ident) {
                return match val {
                    VarValue::Const(num) => Some(num.clone()),
                    VarValue::Alloc(..) => None,
                    VarValue::Func(_) => None,
                };
            }
        }
        None
    }

    pub fn print_scope(&self) {
        for scope in self.scopes.iter() {
            for var in scope.table.symbols.iter() {
                println!("{}", var.ident());
            }
        }
    }
}


/////////////////////////////
// Var 

#[derive(Debug, Clone)]
pub enum Var {
    Normal(NormalVar),
    Const(ConstVar),
    Func(FuncVar),
    Array(ArrayVar),
}

#[derive(Debug, Clone)]
pub struct ArrayVar {
    ident: Ident,
    is_pointer: bool,
    dims: Vec<i32>,
    value: Option<Value>, // alloc in Koopa IR
}

#[derive(Debug, Clone)]
pub enum VarValue {
    Const(i32),
    Alloc(Option<Value>, Option<Vec<i32>>, Option<bool>),
    Func(Option<Function>),
}

#[derive(Debug, Clone)]
pub struct NormalVar {
    ident: Ident,
    value: Option<Value>, // alloc in Koopa IR
}

#[derive(Debug, Clone)]
pub struct ConstVar {
    ident: Ident,
    value: i32,
}

#[derive(Debug, Clone)]
pub struct FuncVar {
    ident: Ident,
    value: Option<Function>,
}


impl Hash for Var {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.ident().hash(state);
    }
}

impl PartialEq for Var {
    fn eq(&self, other: &Self) -> bool {
        self.ident() == other.ident()
    }
}

impl Eq for Var {}


impl Var {
    pub fn new_const(ident: Ident, value: i32) -> Var {
        Var::Const(ConstVar { 
            ident: ident,
            value: value,
        })
    }

    pub fn new_normal(ident: Ident, value : Option<Value>) -> Var {
        Var::Normal(NormalVar {
            ident: ident,
            value: value,
        })
    }

    pub fn new_array(ident: Ident, is_pointer: bool, dims: Vec<i32>, value: Option<Value>) -> Var {
        Var::Array(ArrayVar {
            ident: ident,
            is_pointer: is_pointer,
            dims: dims,
            value: value,
        })
    }

    // return the ident of the var
    fn ident(&self) -> String {
        match self {
            Var::Normal(var) => var.ident.clone(),
            Var::Const(var) => var.ident.clone(),
            Var::Func(var) => var.ident.clone(),
            Var::Array(var) => var.ident.clone(),
        }
    }
    // return true if the var is const
    pub fn is_const(&self) -> bool {
        match self {
            Var::Normal(_) => false,
            Var::Const(_) => true,
            Var::Func(_) => false,
            Var::Array(_) => false,
        }
    }
    // return the const value of the var
    fn const_value(&self) -> Option<i32> {
        match self {
            Var::Normal(_) => None,
            Var::Const(var) => Some(var.value),
            Var::Func(_) => None,
            Var::Array(_) => None,
        }
    }

    // return the value of the var (include the address of the array var)
    fn value(&self) -> Option<Value> {
        match self {
            Var::Normal(var) => var.value.clone(),
            Var::Const(_) => None,
            Var::Func(_) => None,
            Var::Array(var) => var.value.clone(),
        }
    }

    fn func_value(&self) -> Option<Function> {
        match self {
            Var::Normal(_) => None,
            Var::Const(_) => None,
            Var::Func(var) => var.value.clone(),
            Var::Array(_) => None,
        }
    }
}