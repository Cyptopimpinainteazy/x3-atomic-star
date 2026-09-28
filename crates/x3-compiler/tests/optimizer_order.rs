//! The optimizer must not care what order a module's functions come in.
//!
//! It used to. `compiler.rs` moves `main` to function 0 (the executor's entry contract), and it
//! has to do that *after* the optimizer, because moving it first made four of this crate's e2e
//! programs fail with `MIR value MirValue(1) not found in register map` (fib, loop_ops,
//! match_cond, nested conditionals). The cause was PRE keeping one expression table across
//! functions, so what it did to one function depended on which functions it had seen before.
//! #518 rewrote PRE per function and the failure no longer reproduces, but nothing held that in
//! place. This does: every fixture is optimized with its functions in every rotation, and each
//! function's optimized MIR has to be identical to the source-order run.

use std::collections::BTreeMap;

use x3_hir::HirLowerer;
use x3_mir::{MirFunction, MirLowerer, MirModule};
use x3_opt::{OptLevel, Optimizer};

fn lower(source: &str) -> MirModule {
    let ast = x3_parser::parse_program(source).expect("fixture parses");
    let hir = HirLowerer::lower(ast).expect("fixture lowers to HIR");
    MirLowerer::lower(&hir).expect("fixture lowers to MIR")
}

/// Optimize `module` and key the result by function symbol, so two runs over the same functions
/// in different orders can be compared function by function.
fn optimized_by_symbol(mut module: MirModule, level: OptLevel) -> BTreeMap<String, MirFunction> {
    Optimizer::new(level)
        .run(&mut module)
        .expect("the optimizer accepts a fixture it accepted in source order");
    module
        .functions
        .into_iter()
        .map(|function| (format!("{:?}", function.symbol), function))
        .collect()
}

fn fixtures() -> Vec<(String, String)> {
    let dir = format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR"));
    let mut found: Vec<(String, String)> = std::fs::read_dir(&dir)
        .expect("fixture directory exists")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "x3"))
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let source = std::fs::read_to_string(&path).expect("fixture is readable");
            (name, source)
        })
        .collect();
    found.sort();
    found
}

/// Programs with three or more functions, calling each other, so a rotation also changes which
/// function a pass sees between a caller and its callee.
const MULTI_FUNCTION: &[(&str, &str)] = &[
    (
        "three_way_calls",
        r#"
        fn square(x: i64) -> i64 { return x * x; }
        fn sum_squares(a: i64, b: i64) -> i64 {
            let s = square(a) + square(b);
            return s + square(a);
        }
        fn main() -> i64 {
            let t = sum_squares(3, 4);
            return t + sum_squares(3, 4);
        }
    "#,
    ),
    (
        "shared_subexpressions",
        r#"
        fn f(a: i64, b: i64) -> i64 { let x = a + b; let y = a + b; return x * y; }
        fn g(a: i64, b: i64) -> i64 { let x = a + b; if a > b { return x; } return a + b; }
        fn h(n: i64) -> i64 {
            let mut i = 0;
            let mut acc = 0;
            while i < n { acc = acc + (i + 1); i = i + 1; }
            return acc;
        }
        fn main() -> i64 { return f(1, 2) + g(3, 4) + h(5); }
    "#,
    ),
];

fn assert_order_insensitive(name: &str, source: &str) -> usize {
    let module = lower(source);
    let n = module.functions.len();
    for level in [OptLevel::Basic, OptLevel::Default, OptLevel::Aggressive] {
        let reference = optimized_by_symbol(module.clone(), level);
        for shift in 1..n {
            let mut rotated = module.clone();
            rotated.functions.rotate_left(shift);
            let got = optimized_by_symbol(rotated, level);
            assert_eq!(
                got.keys().collect::<Vec<_>>(),
                reference.keys().collect::<Vec<_>>(),
                "{name} ({level:?}, rotated by {shift}): the optimizer changed the function set"
            );
            for (symbol, function) in &reference {
                assert_eq!(
                    &got[symbol], function,
                    "{name} ({level:?}, rotated by {shift}): function {symbol} optimized \
                     differently when the functions came in another order"
                );
            }
        }
    }
    n
}

#[test]
fn every_fixture_optimizes_the_same_in_every_function_order() {
    let fixtures = fixtures();
    assert!(fixtures.len() >= 8, "the fixture corpus is present");
    let mut rotated = 0;
    for (name, source) in &fixtures {
        if assert_order_insensitive(name, source) > 1 {
            rotated += 1;
        }
    }
    assert!(
        rotated >= 7,
        "the check has to reach the multi-function fixtures (reached {rotated})"
    );
}

#[test]
fn programs_with_several_calling_functions_optimize_the_same_in_every_order() {
    for (name, source) in MULTI_FUNCTION {
        assert!(assert_order_insensitive(name, source) >= 3);
    }
}

/// The entry reorder in `compiler.rs` can therefore sit anywhere; what the executor needs is that
/// `main` is function 0 of the emitted module and that the program still compiles and means the
/// same thing. The compiled module is the same bytes whichever order the source declared the
/// functions in, as long as `main` ends up first.
#[test]
fn declaring_main_first_or_last_compiles_to_the_same_module() {
    use x3_compiler::{CompilationOptions, Compiler};

    let helpers = "fn add(a: i64, b: i64) -> i64 { return a + b; }\n\
                   fn twice(x: i64) -> i64 { return add(x, x); }\n";
    let main = "fn main() -> i64 { let v = twice(21); return add(v, 0); }\n";
    for level in [
        OptLevel::None,
        OptLevel::Basic,
        OptLevel::Default,
        OptLevel::Aggressive,
    ] {
        let compile = |source: String| {
            Compiler::compile(
                &source,
                CompilationOptions {
                    opt_level: level,
                    ..Default::default()
                },
            )
            .unwrap_or_else(|e| panic!("{level:?}: {e:?}"))
            .bytecode
            .code
        };
        let main_last = compile(format!("{helpers}{main}"));
        let main_first = compile(format!("{main}{helpers}"));
        assert!(!main_last.is_empty());
        assert_eq!(
            main_first, main_last,
            "{level:?}: where `main` is declared must not change the compiled program"
        );
    }
}
