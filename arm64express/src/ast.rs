use std::collections::HashMap;

pub(crate) type Var = String;
pub(crate) type Val = i32;

pub(crate) struct Node {
    pub(crate) ast: Ast,
    #[allow(dead_code)]
    pub(crate) weight: usize,
    // Actually, with the constant propagation implemented, it is true only for Ast::Literal.
    pub(crate) is_const: bool,
    pub(crate) stack_size: usize,
}

impl Node {
    fn new(ast: Ast, weight: usize, is_const: bool, stack_size: usize) -> Self {
        Self {
            ast,
            weight,
            is_const,
            stack_size,
        }
    }

    pub(crate) fn eval(&self, env: &HashMap<Var, Val>) -> Result<Val, Box<dyn std::error::Error>> {
        self.ast.eval(env)
    }

    pub(crate) fn literal(val: impl Into<Val>) -> Self {
        Self::new(Ast::Literal(val.into()), 0, true, 0)
    }

    pub(crate) fn var(var: impl Into<Var>) -> Self {
        Self::new(Ast::Var(var.into()), 0, false, 0)
    }

    pub(crate) fn add(left: impl Into<Box<Self>>, right: impl Into<Box<Self>>) -> Self {
        Self::binop(Ast::Add, left, right)
    }

    pub(crate) fn sub(left: impl Into<Box<Self>>, right: impl Into<Box<Self>>) -> Self {
        Self::binop(Ast::Sub, left, right)
    }

    pub(crate) fn mul(left: impl Into<Box<Self>>, right: impl Into<Box<Self>>) -> Self {
        Self::binop(Ast::Mul, left, right)
    }

    pub(crate) fn div(left: impl Into<Box<Self>>, right: impl Into<Box<Self>>) -> Self {
        Self::binop(Ast::Div, left, right)
    }

    fn binop(
        op: impl Fn(Box<Self>, Box<Self>) -> Ast,
        left: impl Into<Box<Self>>,
        right: impl Into<Box<Self>>,
    ) -> Self {
        let left = left.into();
        let right = right.into();

        let weight = get_weight(left.weight, right.weight);
        let is_const = get_is_const(left.is_const, right.is_const);
        let stack_size = get_stack_size(left.stack_size, right.stack_size);

        let node = Self::new(op(left, right), weight, is_const, stack_size);

        if is_const {
            if let Ok(val) = node.eval(&<_>::default()) {
                return Node::literal(val);
            }
        }
        // else
        node
    }
}

fn get_weight(left_weight: usize, right_weight: usize) -> usize {
    1 + left_weight + right_weight
}

fn get_is_const(left_is_const: bool, right_is_const: bool) -> bool {
    left_is_const & right_is_const
}

fn get_stack_size(left_size: usize, right_size: usize) -> usize {
    match left_size.cmp(&right_size) {
        std::cmp::Ordering::Less => right_size,
        std::cmp::Ordering::Equal => 1 + left_size,
        std::cmp::Ordering::Greater => left_size,
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
