use std::collections::HashMap;

pub(crate) type Var = String;
pub(crate) type Val = i32;

pub(crate) enum Ast {
    Const(Val),
    Var(Var),
    Add(Box<Ast>, Box<Ast>),
    Sub(Box<Ast>, Box<Ast>),
    Mul(Box<Ast>, Box<Ast>),
    Div(Box<Ast>, Box<Ast>),
}

impl Ast {
    pub(crate) fn eval(&self, env: &HashMap<Var, Val>) -> Result<Val, Box<dyn std::error::Error>> {
        match self {
            Ast::Const(val) => Ok(*val),
            Ast::Var(name) => env.get(name).copied().ok_or("A variable not found".into()),
            Ast::Add(a1, a2) => Ok(a1.eval(env)? + a2.eval(env)?),
            Ast::Sub(a1, a2) => Ok(a1.eval(env)? - a2.eval(env)?),
            Ast::Mul(a1, a2) => Ok(a1.eval(env)? * a2.eval(env)?),
            Ast::Div(a1, a2) => Ok(a1.eval(env)? / a2.eval(env)?),
        }
    }
}
