use crate::ast_df::*;
use super::{env::Env, scope::VarValue};
// Calculate the value of a const expression
pub trait EvalExp {
    fn eval(&self, env: &mut Env) -> i32;
}

impl EvalExp for ConstExp {
    fn eval(&self, env: &mut Env) -> i32 {
        self.exp.eval(env)
    }
}

impl EvalExp for Exp {
    fn eval(&self, env: &mut Env) -> i32 {
        match self {
            Exp::LOrExp(l_or_exp) => l_or_exp.eval(env),
        }
    }
    
}


impl EvalExp for LOrExp {
    fn eval(&self, env: &mut Env) -> i32 {
        match self {
            LOrExp::LAnd(l_and_exp) => l_and_exp.eval(env),
            LOrExp::LOrLAnd(l_or_exp, l_and_exp) => {
                let or_val = l_or_exp.eval(env);
                let and_val = l_and_exp.eval(env);

                (or_val != 0) as i32 | (and_val != 0) as i32
            },
        }
    }
}

impl EvalExp for LAndExp {
    fn eval(&self, env: &mut Env) -> i32 {
        match self {
            LAndExp::Eq(eq_exp) => eq_exp.eval(env),
            LAndExp::LAndEq(l_and_exp, eq_exp) => {
                let and_val = l_and_exp.eval(env);
                let eq_val = eq_exp.eval(env);
                (and_val!= 0) as i32 & (eq_val!= 0) as i32
            },
        }
    }
}

impl EvalExp for EqExp {
    fn eval(&self, env: &mut Env) -> i32 {
        match self {
            EqExp::Rel(rel_exp) => rel_exp.eval(env),
            EqExp::EqRel(eq_exp, eq_op, rel_exp) => {
                let eq_val = eq_exp.eval(env);
                let rel_val = rel_exp.eval(env);
                match eq_op {
                    EqOp::Eq => (eq_val == rel_val) as i32,
                    EqOp::Neq => (eq_val != rel_val) as i32,
                }
            },
        }
    }
}

impl EvalExp for RelExp {
    fn eval(&self, env: &mut Env) -> i32 {
        match self {
            RelExp::Add(add_exp) => add_exp.eval(env),
            RelExp::RelAdd(rel_exp, rel_op, add_exp) => {
                let rel_val = rel_exp.eval(env);
                let add_val = add_exp.eval(env);
                match rel_op {
                    RelOp::Lt => (rel_val < add_val) as i32,
                    RelOp::Gt => (rel_val > add_val) as i32,
                    RelOp::Le => (rel_val <= add_val) as i32,
                    RelOp::Ge => (rel_val >= add_val) as i32,
                }
            },
        }
    }
}

impl EvalExp for AddExp {
    fn eval(&self, env: &mut Env) -> i32 {
        match self {
            AddExp::Mul(mul_exp) => mul_exp.eval(env),
            AddExp::AddMul(add_exp, add_op ,mul_exp) => {
                let add_val = add_exp.eval(env);
                let mul_val = mul_exp.eval(env);
                match add_op {
                    AddOp::Add => add_val + mul_val,
                    AddOp::Sub => add_val - mul_val,
                }
            },
        }
    }
}

impl EvalExp for MulExp {
    fn eval(&self, env: &mut Env) -> i32 {
        match self {
            MulExp::Unary(unary_exp) => unary_exp.eval(env),
            MulExp::MulUnary(mul_exp, mul_op, unary_exp) => {
                let mul_val = mul_exp.eval(env);
                let unary_val = unary_exp.eval(env);
                match mul_op {
                    MulOp::Mul => mul_val * unary_val,
                    MulOp::Div => mul_val / unary_val,
                    MulOp::Mod => mul_val % unary_val,
                }
            },
        }
    }
}

impl EvalExp for UnaryExp {
    fn eval(&self, env: &mut Env) -> i32 {
        match self {
            UnaryExp::PrimaryExp(primary_exp) => primary_exp.eval(env),
            UnaryExp::Unary(unary_op, unary_exp) => {
                let unary_val = unary_exp.eval(env);
                match unary_op {
                    UnaryOp::Plus => unary_val,
                    UnaryOp::Minus => -unary_val,
                    UnaryOp::Not => (unary_val == 0) as i32,
                }
            },
            UnaryExp::FuncCall(_) => panic!("Function call not implemented"),
        }
    }
}

impl EvalExp for PrimaryExp {
    fn eval(&self, env: &mut Env) -> i32 {
        match self {
            PrimaryExp::Num(num) => num.clone(),
            PrimaryExp::Exp(exp) => {
                exp.eval(env)
            }
            PrimaryExp::LVal(lval) => {
                match lval {
                    LVal::Ident(ident) => {
                        if let Some(val) = env.scope.lookup_var(ident) {
                            match val {
                                VarValue::Const(num) => num,
                                VarValue::Alloc(..) => panic!("Variable {} is not const", ident),
                                VarValue::Func(_) => panic!("Variable {} is a function", ident),
                            }
                        }
                        else {
                            panic!("Variable {} not found in this scope", ident)
                        }   
                    }
                    LVal::Array(..) => panic!("Array does not occur in the constexpr"),
                }
            }
        }
    }
}