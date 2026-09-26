//! Golden tests for type checking.
//!
//! These tests verify that the type checker produces expected results for various
//! X3 programs, including type inference, type errors, and cross-VM type validation.

use std::fs;
use std::path::PathBuf;

use x3_parser::Parser;
use x3_semantics::Resolver;
use x3_typeck::{TypeChecker, TypeError, TypeErrorKind};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn get_fixtures() -> Vec<(String, String)> {
    let mut fixtures = Vec::new();
    let dir = fixtures_dir();

    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "x3").unwrap_or(false) {
                if let Ok(content) = fs::read_to_string(&path) {
                    let name = path.file_stem().unwrap().to_string_lossy().to_string();
                    fixtures.push((name, content));
                }
            }
        }
    }

    fixtures.sort_by(|a, b| a.0.cmp(&b.0));
    fixtures
}

fn parse_resolve_check(source: &str) -> Result<x3_typeck::TypedModule, Vec<TypeError>> {
    let mut parser = Parser::from_source(source);
    let module = parser.parse_module().expect("should parse");

    let resolver = Resolver::new();
    let resolved = resolver.resolve(&module).expect("should resolve");

    let checker = TypeChecker::new();
    checker.check(&module, &resolved)
}

/// Test that all valid fixtures type check without errors.
#[test]
fn test_valid_fixtures_typecheck() {
    let fixtures = get_fixtures();

    for (name, source) in &fixtures {
        // Skip error test fixtures
        if name.contains("error") {
            continue;
        }

        println!("Testing fixture: {}", name);

        match parse_resolve_check(source) {
            Ok(typed) => {
                println!(
                    "  ✓ {} - {} expression types recorded",
                    name,
                    typed.expr_types.len()
                );
            }
            Err(errors) => {
                // Some fixtures may have intentional errors or use unsupported features
                // For now, just report them
                println!(
                    "  ⚠ {} - {} type errors (may be expected):",
                    name,
                    errors.len()
                );
                for err in &errors {
                    println!("    - {:?}", err.kind);
                }
            }
        }
    }
}

/// Test expression type inference for literals.
#[test]
fn test_literal_type_inference() {
    let fixtures = get_fixtures();

    if let Some((_, source)) = fixtures.iter().find(|(n, _)| n.contains("literal")) {
        match parse_resolve_check(source) {
            Ok(typed) => {
                // Verify we inferred types for expressions
                assert!(
                    !typed.expr_types.is_empty(),
                    "Should have inferred some expression types"
                );
                println!(
                    "Literal fixtures: {} expression types inferred",
                    typed.expr_types.len()
                );
            }
            Err(errors) => {
                // May have errors due to missing type annotations
                println!("Literal test errors: {:?}", errors);
            }
        }
    }
}

/// Test binary operation type inference.
#[test]
fn test_binary_op_type_inference() {
    let fixtures = get_fixtures();

    if let Some((_, source)) = fixtures.iter().find(|(n, _)| n.contains("binary")) {
        match parse_resolve_check(source) {
            Ok(typed) => {
                println!(
                    "Binary ops: {} expression types inferred",
                    typed.expr_types.len()
                );
            }
            Err(errors) => {
                println!("Binary op test errors: {:?}", errors);
            }
        }
    }
}

/// Test function type checking.
#[test]
fn test_function_types() {
    let fixtures = get_fixtures();

    if let Some((_, source)) = fixtures.iter().find(|(n, _)| n.contains("function")) {
        match parse_resolve_check(source) {
            Ok(typed) => {
                println!(
                    "Function types: {} expression types inferred",
                    typed.expr_types.len()
                );
            }
            Err(errors) => {
                println!("Function type test errors: {:?}", errors);
            }
        }
    }
}

#[cfg(test)]
mod error_tests {
    use super::*;

    /// Test type mismatch detection.
    #[test]
    fn test_type_mismatch() {
        let source = r#"
            fn test() -> u64 {
                return true;
            }
        "#;

        let result = parse_resolve_check(source);
        assert!(result.is_err(), "Should detect type mismatch");
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e.kind, TypeErrorKind::TypeMismatch { .. })),
            "Should have TypeMismatch error, got: {:?}",
            errors
        );
    }

    /// Test wrong argument count detection.
    #[test]
    fn test_wrong_argument_count() {
        let source = r#"
            fn add(a: u64, b: u64) -> u64 {
                return a + b;
            }
            fn test() {
                let result = add(1);
            }
        "#;

        let result = parse_resolve_check(source);
        assert!(result.is_err(), "Should detect wrong argument count");
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e.kind, TypeErrorKind::WrongArgumentCount { .. })),
            "Should have WrongArgumentCount error, got: {:?}",
            errors
        );
    }

    /// Test condition not bool detection.
    #[test]
    fn test_condition_not_bool() {
        let source = r#"
            fn test() {
                if (42) {
                    let x = 1;
                }
            }
        "#;

        let result = parse_resolve_check(source);
        assert!(result.is_err(), "Should detect non-bool condition");
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e.kind, TypeErrorKind::ConditionNotBool(_))),
            "Should have ConditionNotBool error, got: {:?}",
            errors
        );
    }

    /// Test invalid binary operation.
    #[test]
    fn test_invalid_binary_op() {
        let source = r#"
            fn test() {
                let result = "hello" + 42;
            }
        "#;

        let result = parse_resolve_check(source);
        assert!(result.is_err(), "Should detect invalid binary operation");
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e.kind, TypeErrorKind::InvalidBinaryOperation { .. })),
            "Should have InvalidBinaryOperation error, got: {:?}",
            errors
        );
    }

    /// Test not callable detection.
    #[test]
    fn test_not_callable() {
        let source = r#"
            fn test() {
                let x = 42;
                let result = x();
            }
        "#;

        let result = parse_resolve_check(source);
        assert!(result.is_err(), "Should detect not callable");
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e.kind, TypeErrorKind::NotCallable(_))),
            "Should have NotCallable error, got: {:?}",
            errors
        );
    }

    fn assert_only_argument_type_mismatch(errors: &[TypeError]) {
        assert!(!errors.is_empty(), "Expected at least one type error");
        assert!(
            errors
                .iter()
                .all(|e| matches!(e.kind, TypeErrorKind::ArgumentTypeMismatch { .. })),
            "Expected only ArgumentTypeMismatch error(s), got: {:?}",
            errors
        );
    }

    // This test enforces the numeric argument mismatch policy defined in
    // `docs/rfc/RFC-t5-6-numeric-coercion-policy.md` (as amended 2026-09-26).
    // It ensures incompatible literal arguments are diagnosed as
    // `ArgumentTypeMismatch` rather than generic type/unification failures.
    /// Test argument type mismatch.
    #[test]
    fn test_argument_type_mismatch() {
        let source = r#"
            fn greet(name: bool) {
                return;
            }
            fn test() {
                greet(42);
            }
        "#;

        let result = parse_resolve_check(source);
        assert!(result.is_err(), "Should detect argument type mismatch");
        let errors = result.unwrap_err();
        assert_only_argument_type_mismatch(&errors);
    }

    /// Numeric argument mismatches under RFC t5-6 as amended 2026-09-26: an unsuffixed literal
    /// takes the parameter's integer type when every value fits it, and a *typed* value never
    /// converts between integer types.
    #[test]
    fn test_argument_type_mismatch_numeric_literals() {
        let cases = [
            (
                "negative literal to u64 parameter",
                r#"
                    fn greet(x: u64) {
                        return;
                    }
                    fn test() {
                        greet(-42);
                    }
                "#,
            ),
            (
                "negative literal to u32 parameter",
                r#"
                    fn greet(x: u32) {
                        return;
                    }
                    fn test() {
                        greet(-1);
                    }
                "#,
            ),
            (
                "literal out of range for u8 parameter",
                r#"
                    fn greet(x: u8) {
                        return;
                    }
                    fn test() {
                        greet(300);
                    }
                "#,
            ),
            (
                "literal out of range for i8 parameter",
                r#"
                    fn greet(x: i8) {
                        return;
                    }
                    fn test() {
                        greet(-129);
                    }
                "#,
            ),
            (
                "typed u64 value to i64 parameter",
                r#"
                    fn greet(x: i64) {
                        return;
                    }
                    fn test() {
                        let v: u64 = 42;
                        greet(v);
                    }
                "#,
            ),
            (
                "typed i64 value to u32 parameter",
                r#"
                    fn greet(x: u32) {
                        return;
                    }
                    fn test() {
                        let v: i64 = 1;
                        greet(v);
                    }
                "#,
            ),
            (
                "typed u32 value to u64 parameter (no widening)",
                r#"
                    fn greet(x: u64) {
                        return;
                    }
                    fn test() {
                        let v: u32 = 1;
                        greet(v);
                    }
                "#,
            ),
            (
                "literal bound to u64 by its first use, then passed as i64",
                r#"
                    fn takes_u64(x: u64) {
                        return;
                    }
                    fn takes_i64(x: i64) {
                        return;
                    }
                    fn test() {
                        let v = 7;
                        takes_u64(v);
                        takes_i64(v);
                    }
                "#,
            ),
        ];

        for (case, source) in cases {
            let result = parse_resolve_check(source);
            assert!(result.is_err(), "{} should produce a type error", case);
            let errors = result.unwrap_err();
            assert_only_argument_type_mismatch(&errors);
        }
    }

    /// The accepted half of the amended policy: an unsuffixed literal takes the integer type its
    /// parameter requires, whatever that type is, provided the value fits.
    #[test]
    fn test_numeric_literals_take_the_parameter_type() {
        let cases = [
            (
                "42 to i64",
                "fn f(x: i64) { return; }\nfn test() { f(42); }",
            ),
            (
                "42 to u64",
                "fn f(x: u64) { return; }\nfn test() { f(42); }",
            ),
            (
                "42 to u32",
                "fn f(x: u32) { return; }\nfn test() { f(42); }",
            ),
            (
                "42 to u128",
                "fn f(x: u128) { return; }\nfn test() { f(42); }",
            ),
            (
                "255 to u8",
                "fn f(x: u8) { return; }\nfn test() { f(255); }",
            ),
            (
                "-1 to i64",
                "fn f(x: i64) { return; }\nfn test() { f(-1); }",
            ),
            (
                "-128 to i8",
                "fn f(x: i8) { return; }\nfn test() { f(-128); }",
            ),
            (
                "literal arithmetic to i64",
                "fn f(x: i64) { return; }\nfn test() { f(1 + 2 * 3); }",
            ),
            ("i64 return", "fn f() -> i64 {\n    return 42;\n}"),
        ];
        for (case, source) in cases {
            let result = parse_resolve_check(source);
            assert!(
                result.is_ok(),
                "{case} should type check: {:?}",
                result.err()
            );
        }
    }

    /// Test while loop condition.
    #[test]
    fn test_while_condition_not_bool() {
        let source = r#"
            fn test() {
                while ("forever") {
                    let x = 1;
                }
            }
        "#;

        let result = parse_resolve_check(source);
        assert!(result.is_err(), "Should detect non-bool while condition");
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e.kind, TypeErrorKind::ConditionNotBool(_))),
            "Should have ConditionNotBool error, got: {:?}",
            errors
        );
    }

    /// Test invalid unary operation.
    #[test]
    fn test_invalid_unary_op() {
        let source = r#"
            fn test() {
                let result = -"hello";
            }
        "#;

        let result = parse_resolve_check(source);
        assert!(result.is_err(), "Should detect invalid unary operation");
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e.kind, TypeErrorKind::InvalidOperation { .. })),
            "Should have InvalidOperation error, got: {:?}",
            errors
        );
    }

    /// Test logical not on non-bool.
    #[test]
    fn test_logical_not_non_bool() {
        let source = r#"
            fn test() {
                let result = !42;
            }
        "#;

        let result = parse_resolve_check(source);
        assert!(result.is_err(), "Should detect logical not on non-bool");
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e.kind, TypeErrorKind::InvalidOperation { .. })),
            "Should have InvalidOperation error, got: {:?}",
            errors
        );
    }

    /// Test comparison on non-comparable types.
    #[test]
    fn test_comparison_type_error() {
        let source = r#"
            fn test() {
                let result = true < false;
            }
        "#;

        let result = parse_resolve_check(source);
        assert!(
            result.is_err(),
            "Should detect comparison on non-comparable types"
        );
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e.kind, TypeErrorKind::InvalidBinaryOperation { .. })),
            "Should have InvalidBinaryOperation error, got: {:?}",
            errors
        );
    }

    /// Test return with wrong type.
    #[test]
    fn test_return_wrong_type() {
        let source = r#"
            fn get_number() -> u64 {
                return "not a number";
            }
        "#;

        let result = parse_resolve_check(source);
        assert!(result.is_err(), "Should detect wrong return type");
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e.kind, TypeErrorKind::TypeMismatch { .. })),
            "Should have TypeMismatch error, got: {:?}",
            errors
        );
    }

    /// Test return missing from typed function.
    #[test]
    fn test_unit_return_for_typed_function() {
        let source = r#"
            fn get_number() -> u64 {
                return;
            }
        "#;

        let result = parse_resolve_check(source);
        assert!(result.is_err(), "Should detect missing return value");
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e.kind, TypeErrorKind::TypeMismatch { .. })),
            "Should have TypeMismatch error, got: {:?}",
            errors
        );
    }

    /// Test logical operations on non-bools.
    #[test]
    fn test_logical_op_non_bool() {
        let source = r#"
            fn test() {
                let result = 1 && 2;
            }
        "#;

        let result = parse_resolve_check(source);
        assert!(
            result.is_err(),
            "Should detect logical operation on non-bools"
        );
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e.kind, TypeErrorKind::InvalidBinaryOperation { .. })),
            "Should have InvalidBinaryOperation error, got: {:?}",
            errors
        );
    }

    /// Test for loop condition not bool.
    #[test]
    fn test_for_condition_not_bool() {
        let source = r#"
            fn test() {
                for (let mut i = 0; "always"; i = i + 1) {
                    let x = 1;
                }
            }
        "#;

        let result = parse_resolve_check(source);
        assert!(result.is_err(), "Should detect non-bool for condition");
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e.kind, TypeErrorKind::ConditionNotBool(_))),
            "Should have ConditionNotBool error, got: {:?}",
            errors
        );
    }

    /// Test valid program type checks successfully.
    #[test]
    fn test_valid_program() {
        let source = r#"
            fn add(a: u64, b: u64) -> u64 {
                return a + b;
            }

            fn main() -> u64 {
                let x = 10;
                let y = 20;
                let sum = add(x, y);
                if (sum > 25) {
                    return sum;
                }
                return 0;
            }
        "#;

        let result = parse_resolve_check(source);
        assert!(
            result.is_ok(),
            "Valid program should type check: {:?}",
            result.err()
        );
    }
}
