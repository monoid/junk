mod ast;
mod codegen;
mod parser;

use crate::codegen::Function;

fn main() {
    let ast = parser::parse("(((5)+42)*8)").unwrap().1;
    println!("{}", ast.eval(&Default::default()).unwrap());

    let ast = parser::parse("8-2-2").unwrap().1;
    println!("{}", ast.eval(&Default::default()).unwrap());

    let func = Function::new();
    println!("{}", func.call(80, 1));
}
