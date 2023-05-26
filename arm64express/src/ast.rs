use std::collections::HashMap;

pub(crate) type Var = String;
pub(crate) type Val = i32;

pub(crate) struct Node {
    ast: Ast,
    #[allow(dead_code)]
    weight: usize,
}

impl Node {
    fn new(ast: Ast) -> Self {
        Self { ast, weight: 0 }
    }

    pub(crate) fn eval(&self, env: &HashMap<Var, Val>) -> Result<Val, Box<dyn std::error::Error>> {
        self.ast.eval(env)
    }

    pub(crate) fn literal(val: impl Into<Val>) -> Self {
        Self::new(Ast::Literal(val.into()))
    }

    pub(crate) fn var(var: impl Into<Var>) -> Self {
        Self::new(Ast::Var(var.into()))
    }

    pub(crate) fn add(left: impl Into<Box<Self>>, right: impl Into<Box<Self>>) -> Self {
        Self::new(Ast::Add(left.into(), right.into()))
    }

    pub(crate) fn sub(left: impl Into<Box<Self>>, right: impl Into<Box<Self>>) -> Self {
        Self::new(Ast::Sub(left.into(), right.into()))
    }

    pub(crate) fn mul(left: impl Into<Box<Self>>, right: impl Into<Box<Self>>) -> Self {
        Self::new(Ast::Mul(left.into(), right.into()))
    }

    pub(crate) fn div(left: impl Into<Box<Self>>, right: impl Into<Box<Self>>) -> Self {
        Self::new(Ast::Div(left.into(), right.into()))
    }
}

pub(crate) enum Ast {
    Literal(Val),
    Var(Var),
    Add(Box<Node>, Box<Node>),
    Sub(Box<Node>, Box<Node>),
    Mul(Box<Node>, Box<Node>),
    Div(Box<Node>, Box<Node>),
}

impl Ast {
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
