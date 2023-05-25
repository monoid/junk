mod ast;
mod parser;

fn main() {
    let ast = parser::parse("(((5)+42)*8)").unwrap().1;
    println!("{}", ast.eval(&Default::default()).unwrap());

    let ast = parser::parse("8-2-2").unwrap().1;
    println!("{}", ast.eval(&Default::default()).unwrap());
}
