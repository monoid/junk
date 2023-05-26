use std::collections::HashMap;

pub(crate) type Var = String;
pub(crate) type Val = i32;

pub(crate) enum Ast {
    Literal(Val),
    Var(Var),
    Add(Box<Ast>, Box<Ast>),
    Sub(Box<Ast>, Box<Ast>),
    Mul(Box<Ast>, Box<Ast>),
    Div(Box<Ast>, Box<Ast>),
}

impl Ast {
    pub(crate) fn literal(val: Val) -> Self {
        Self::Literal(val)
    }

    pub(crate) fn var(var: Var) -> Self {
        Self::Var(var)
    }

    pub(crate) fn add(left: impl Into<Box<Ast>>, right: impl Into<Box<Ast>>) -> Self {
        Self::Add(left.into(), right.into())
    }

    pub(crate) fn sub(left: impl Into<Box<Ast>>, right: impl Into<Box<Ast>>) -> Self {
        Self::Sub(left.into(), right.into())
    }

    pub(crate) fn mul(left: impl Into<Box<Ast>>, right: impl Into<Box<Ast>>) -> Self {
        Self::Mul(left.into(), right.into())
    }

    pub(crate) fn div(left: impl Into<Box<Ast>>, right: impl Into<Box<Ast>>) -> Self {
        Self::Div(left.into(), right.into())
    }

    pub(crate) fn eval(&self, env: &HashMap<Var, Val>) -> Result<Val, Box<dyn std::error::Error>> {
        match self {
            Ast::Literal(val) => Ok(*val),
            Ast::Var(name) => env.get(name).copied().ok_or("A variable not found".into()),
            Ast::Add(a1, a2) => Ok(a1.eval(env)? + a2.eval(env)?),
            Ast::Sub(a1, a2) => Ok(a1.eval(env)? - a2.eval(env)?),
            Ast::Mul(a1, a2) => Ok(a1.eval(env)? * a2.eval(env)?),
            Ast::Div(a1, a2) => Ok(a1.eval(env)? / a2.eval(env)?),
        }
    }
}
