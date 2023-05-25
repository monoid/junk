use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::combinator::{map, value};
use nom::multi::many0;
use nom::IResult;

use crate::ast;

pub(crate) fn parse(inp: &str) -> IResult<&str, ast::Ast> {
    use nom::combinator::all_consuming;

    all_consuming(parse_expr)(inp)
}

pub(crate) fn parse_expr(inp: &str) -> IResult<&str, ast::Ast> {
    parse_add(inp)
}

pub(crate) fn parse_add(inp: &str) -> IResult<&str, ast::Ast> {
    use nom::sequence::pair;

    #[derive(Clone)]
    enum AddOp {
        Add,
        Sub,
    }

    map(
        pair(
            parse_prod,
            many0(pair(
                alt((value(AddOp::Add, tag("+")), value(AddOp::Sub, tag("-")))),
                parse_prod,
            )),
        ),
        |(left, rights)| {
            rights
                .into_iter()
                .fold(left, |left, right_op| match right_op {
                    (AddOp::Add, right) => ast::Ast::Add(left.into(), right.into()),
                    (AddOp::Sub, right) => ast::Ast::Sub(left.into(), right.into()),
                })
        },
    )(inp)
}

pub(crate) fn parse_prod(inp: &str) -> IResult<&str, ast::Ast> {
    use nom::sequence::pair;

    #[derive(Clone)]
    enum ProdOp {
        Mul,
        Div,
    }

    map(
        pair(
            parse_atom,
            many0(pair(
                alt((value(ProdOp::Mul, tag("*")), value(ProdOp::Div, tag("/")))),
                parse_prod,
            )),
        ),
        |(left, rights)| {
            rights
                .into_iter()
                .fold(left, |left, right_op| match right_op {
                    (ProdOp::Mul, second) => ast::Ast::Mul(left.into(), second.into()),
                    (ProdOp::Div, second) => ast::Ast::Div(left.into(), second.into()),
                })
        },
    )(inp)
}

fn parse_atom(inp: &str) -> IResult<&str, ast::Ast> {
    use nom::character::complete::alpha1;
    use nom::character::complete::i32;
    use nom::sequence::delimited;

    alt((
        map(i32, ast::Ast::Const),
        map(alpha1, |s: &str| ast::Ast::Var(s.into())),
        delimited(tag("("), parse_expr, tag(")")),
    ))(inp)
}
