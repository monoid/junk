mod ast;
mod codegen;
mod parser;

use crate::{ast::Ast, codegen::Function};

fn main() {
    let node = parser::parse("(((5)+42)*8)").unwrap().1;
    assert!(
        matches!(node.ast, Ast::Literal(_)),
        "constant propagation failed",
    );
    println!("{}", node.eval(&Default::default()).unwrap());

    let node = parser::parse("8-2-2").unwrap().1;
    assert!(
        matches!(node.ast, Ast::Literal(_)),
        "constant propagation failed",
    );
    println!("{}", node.eval(&Default::default()).unwrap());

    let node = parser::parse("a+b+c+d+e").unwrap().1;
    assert_eq!(
        node.stack_size, 1
    );

    let node = parser::parse("a*b*c*d*e").unwrap().1;
    assert_eq!(
        node.stack_size, 1
    );

    let node = parser::parse("(a+b)+(c+d)*e").unwrap().1;
    assert_eq!(
        node.stack_size, 2
    );

    let node = parser::parse("a+b+c*d+e").unwrap().1;
    assert_eq!(
        node.stack_size, 2
    );


    let func = Function::new();
    println!("{}", func.call(80, 1));
}
