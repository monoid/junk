use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::combinator::{map, value};
use nom::multi::many0;
use nom::IResult;

use crate::ast;

pub(crate) fn parse(inp: &str) -> IResult<&str, ast::Node> {
    use nom::combinator::all_consuming;

    all_consuming(parse_expr)(inp)
}

pub(crate) fn parse_expr(inp: &str) -> IResult<&str, ast::Node> {
    parse_add(inp)
}

pub(crate) fn parse_add(inp: &str) -> IResult<&str, ast::Node> {
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
                    (AddOp::Add, right) => ast::Node::add(left, right),
                    (AddOp::Sub, right) => ast::Node::sub(left, right),
                })
        },
    )(inp)
}

pub(crate) fn parse_prod(inp: &str) -> IResult<&str, ast::Node> {
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
                    (ProdOp::Mul, second) => ast::Node::mul(left, second),
                    (ProdOp::Div, second) => ast::Node::div(left, second),
                })
        },
    )(inp)
}

fn parse_atom(inp: &str) -> IResult<&str, ast::Node> {
    use nom::character::complete::alpha1;
    use nom::character::complete::i32;
    use nom::sequence::delimited;

    alt((
        map(i32, ast::Node::literal),
        map(alpha1, ast::Node::var),
        delimited(tag("("), parse_expr, tag(")")),
    ))(inp)
}
