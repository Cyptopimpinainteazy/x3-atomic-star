use x3_lexer::Lexer;

fn main() {
    let source = "for (let k in 0..10) { emit k; }";
    let tokens = Lexer::lex_all(source);
    for token in &tokens {
        println!("{:?}", token);
    }
}
