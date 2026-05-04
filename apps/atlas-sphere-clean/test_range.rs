use x3_parser::Parser;

fn main() {
    let source = "for (let k in 0..10) { emit k; }";
    let mut parser = Parser::from_source(source);
    match parser.parse_module() {
        Ok(module) => println!("Parsed successfully: {:?}", module),
        Err(e) => println!("Parse error: {:?}", e),
    }
}
