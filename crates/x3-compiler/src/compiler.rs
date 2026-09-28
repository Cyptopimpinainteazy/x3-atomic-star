//! Full X3 compiler pipeline orchestration
//!
//! Complete pipeline: Source → Lexer → Parser → HIR → MIR → Optimizer → Bytecode

use x3_backend::bc_format::FeatureFlags;
use x3_backend::BytecodeModule;
use x3_hir::{HirLowerer, HirModule};
use x3_mir::{MirLowerer, MirModule};
use x3_opt::{OptLevel, OptStats, Optimizer};
use x3_verifier::{GasAnalyzer, GasReport, SafetyRules, VerificationReport, Verifier};

use crate::error::{CompilerError, CompilerResult};
use crate::options::CompilationOptions;

/// Compilation artifacts for debugging and analysis
#[derive(Clone, Debug)]
pub struct CompilationArtifacts {
    /// Source code (if retained)
    pub source: Option<String>,
    /// HIR module (if requested)
    pub hir: Option<HirModule>,
    /// MIR module before optimization
    pub mir_unoptimized: Option<MirModule>,
    /// MIR module after optimization
    pub mir_optimized: Option<MirModule>,
    /// Optimization statistics
    pub opt_stats: Option<OptStats>,
    /// Gas analysis report
    pub gas_report: Option<GasReport>,
    /// Contract verification report
    pub verification_report: Option<VerificationReport>,
    /// Final bytecode
    pub bytecode: BytecodeModule,
}

/// Compilation result with optional artifacts
#[derive(Clone, Debug)]
pub struct CompilationOutput {
    /// Final bytecode module
    pub bytecode: BytecodeModule,
    /// Optional artifacts for debugging
    pub artifacts: Option<CompilationArtifacts>,
}

/// Main compiler that orchestrates the full pipeline
pub struct Compiler;

impl Compiler {
    /// Compile X3 source code to optimized bytecode (full pipeline)
    ///
    /// This is the primary entry point for compilation:
    /// Source → Parser → HIR → MIR → Optimizer → Bytecode
    pub fn compile(source: &str, options: CompilationOptions) -> CompilerResult<CompilationOutput> {
        if options.verbose {
            eprintln!("🔧 X3 Compiler v0.1.0");
            eprintln!("  → Optimization level: {:?}", options.opt_level);
        }

        // Phase 1: Parse source to AST
        if options.verbose {
            eprintln!("  [1/5] Parsing source...");
        }
        // Control characters other than tab, newline and carriage return are refused. The lexer
        // skipped them, so a NUL or backspace inside a statement changed nothing the compiler
        // reported while hiding what the source shows a reader (found by the `compile_and_run`
        // fuzz target, whose inputs carried them into a program that then compiled).
        if let Some((offset, ch)) = source
            .char_indices()
            .find(|(_, c)| c.is_control() && !matches!(c, '\t' | '\n' | '\r'))
        {
            return Err(CompilerError::Lexer(format!(
                "control character U+{:04X} at byte {offset} is not allowed in source",
                ch as u32
            )));
        }
        let ast = x3_parser::parse_program(source)
            .map_err(|e| CompilerError::Parser(format!("{:?}", e)))?;

        // Phase 1b: resolve names and check types. The resolver and the type checker existed and
        // were never called — `CompilerError::TypeCheck` had no producer — so a program adding a
        // bool to an integer, returning a bool from an `i64` function, passing `true` for an `i64`
        // parameter, branching on an integer or falling off the end of a function with a return
        // type compiled, and ran with whatever meaning the VM gave the bytes.
        let resolved = x3_semantics::Resolver::new()
            .resolve(&ast)
            .map_err(|errors| CompilerError::TypeCheck(format!("{errors:?}")))?;
        x3_typeck::TypeChecker::new()
            .check(&ast, &resolved)
            .map_err(|errors| CompilerError::TypeCheck(format!("{errors:?}")))?;

        // Where `main` sits among the functions, computed here because the AST is consumed by the
        // HIR lowering below and the HIR keeps the same order (TICKET-130).
        let entry_function_index = ast
            .items
            .iter()
            .filter_map(|item| match item {
                x3_ast::Item::Function(function) => Some(function),
                _ => None,
            })
            .position(|function| function.name.name == "main");

        // The runtime executes function 0 with no arguments, and this pipeline puts `main` there.
        // A program with no `main` compiled to a module the chain refuses (`FunctionNotFound`, found
        // by the `compile_and_run` fuzz target on the empty program), and a `main` with parameters
        // to one it refuses too (the entry would run on registers no caller wrote). Both are the
        // program's error, so they are reported here, against the source.
        let Some(entry) = entry_function_index else {
            return Err(CompilerError::TypeCheck(
                "a program must declare `fn main()`: it is the function the chain executes".into(),
            ));
        };
        let main_params = ast
            .items
            .iter()
            .filter_map(|item| match item {
                x3_ast::Item::Function(function) => Some(function),
                _ => None,
            })
            .nth(entry)
            .map(|function| function.params.len())
            .unwrap_or(0);
        if main_params != 0 {
            return Err(CompilerError::TypeCheck(format!(
                "`main` takes {main_params} parameter(s), but the chain calls it with none"
            )));
        }

        // Phase 2: Lower AST to HIR
        if options.verbose {
            eprintln!("  [2/5] Lowering to HIR...");
        }
        let hir =
            HirLowerer::lower(ast).map_err(|e| CompilerError::HirGeneration(format!("{:?}", e)))?;

        // Phase 3: Lower HIR to MIR
        if options.verbose {
            eprintln!("  [3/5] Lowering to MIR...");
        }
        let mut mir_unoptimized =
            MirLowerer::lower(&hir).map_err(|e| CompilerError::MirLowering(format!("{:?}", e)))?;

        // The runtime's entry contract is the **function index**: `X3Executor::execute` calls
        // function 0 as `main`, and the module format carries no entry-field to say otherwise. The
        // lowering kept source order, so a program that declared a helper before `main` put the
        // helper first and the executor called it instead — measured, `add(a: i64, b: i64)` followed
        // by `main` failed with `ArgumentCountMismatch(2, 0)`, the callee being `add` (TICKET-130).
        //
        // `main` is the entry the gateway lowering already looks for, so it is the one moved to the
        // front. The HIR keeps the AST's function order (both lower `Item::Function` in order), which
        // is what makes the index the same on both sides.

        // Phase 4: Optimize MIR
        if options.verbose {
            eprintln!("  [4/6] Running optimizer ({:?})...", options.opt_level);
        }
        let (mir_optimized, opt_stats) = Self::optimize_mir(&mir_unoptimized, &options)?;

        // The contract being met is about the emitted module's function table, which is built from
        // this order, so reordering here is enough. It once *had* to be here: moving the entry in
        // front of the optimizer made `fib.x3` fail with `MIR value MirValue(1) not found in
        // register map` (four of this crate's e2e tests), because PRE kept one expression table
        // across functions. PRE is per function now, and `tests/optimizer_order.rs` requires every
        // fixture to optimize identically in every function order, so the position of this
        // reorder is no longer load-bearing.
        let mut mir_optimized = mir_optimized;
        if let Some(entry) = entry_function_index {
            if entry < mir_optimized.functions.len() {
                let main = mir_optimized.functions.remove(entry);
                mir_optimized.functions.insert(0, main);
            }
        }

        // Phase 5: Gas analysis and verification (optional)
        let (gas_report, verification_report) = if options.analyze_gas || options.verify_contract {
            if options.verbose {
                eprintln!("  [5/6] Running contract analysis...");
            }
            Self::analyze_contract(&mir_optimized, &options)?
        } else {
            (None, None)
        };

        // Phase 6: Generate bytecode
        if options.verbose {
            eprintln!("  [6/6] Generating bytecode...");
        }
        let mut bytecode =
            x3_backend::MirBytecodeCompiler::compile_with_options(&mir_optimized, options.debug)
                .map_err(|e| CompilerError::Backend(format!("{:?}", e)))?;
        Self::record_compiled_capabilities(&mut bytecode, &options);

        if options.verbose {
            eprintln!("  ✓ Compilation complete");
            eprintln!("    • Functions: {}", bytecode.functions.len());
            eprintln!("    • Code size: {} bytes", bytecode.code.len());

            // Print gas analysis summary if available
            if let Some(ref gas) = gas_report {
                eprintln!("    📊 Gas analysis:");
                eprintln!("       • Total estimated gas: {}", gas.total_gas);
                eprintln!("       • Has unbounded loops: {}", gas.has_unbounded);
                if !gas.exceeds_limit.is_empty() {
                    eprintln!(
                        "       • ⚠️ Functions exceeding limits: {:?}",
                        gas.exceeds_limit
                    );
                }
            }
        }

        let retain_artifacts = options.emit_mir
            || options.emit_hir
            || options.emit_stats
            || options.analyze_gas
            || options.verify_contract;

        let artifacts = if retain_artifacts {
            Some(CompilationArtifacts {
                source: Some(source.to_string()),
                hir: if options.emit_hir { Some(hir) } else { None },
                mir_unoptimized: if options.emit_mir {
                    Some(mir_unoptimized)
                } else {
                    None
                },
                mir_optimized: if options.emit_mir {
                    Some(mir_optimized)
                } else {
                    None
                },
                opt_stats: if options.emit_stats { opt_stats } else { None },
                gas_report,
                verification_report,
                bytecode: bytecode.clone(),
            })
        } else {
            None
        };

        Ok(CompilationOutput {
            bytecode,
            artifacts,
        })
    }

    /// Compile MIR to optimized bytecode (for pre-built MIR)
    ///
    /// Use this when you already have MIR and just need optimization + codegen.
    pub fn compile_mir(
        mir: &MirModule,
        options: CompilationOptions,
    ) -> CompilerResult<BytecodeModule> {
        let (optimized_mir, _stats) = Self::optimize_mir(mir, &options)?;

        // Emit optimized bytecode
        let mut bytecode =
            x3_backend::MirBytecodeCompiler::compile_with_options(&optimized_mir, options.debug)
                .map_err(|e| CompilerError::Backend(format!("{:?}", e)))?;
        Self::record_compiled_capabilities(&mut bytecode, &options);

        Ok(bytecode)
    }

    /// Record the capabilities the compiled policy demands into the module's feature word.
    ///
    /// The pipeline above compiles *what the program does*; this records *what the program
    /// requires* to be allowed to run at all. Keeping it separate matters: an artifact that states
    /// a demand is only worth anything if the loader that reads the header refuses when the chain
    /// cannot meet it, which is the runtime's half of the contract
    /// (`x3-integration::mini_x3` on a block, `x3-vm` off it) and `pallet-x3-kernel`'s at intake.
    ///
    /// `features.set` is used rather than `set_features` on purpose: the latter recomputes
    /// `min_version` from the flag word, and this demand is not a format change — raising the
    /// required version would make the module unreadable by the very loader that is supposed to
    /// interpret the demand.
    fn record_compiled_capabilities(bytecode: &mut BytecodeModule, options: &CompilationOptions) {
        if options.require_private_submission {
            bytecode
                .features
                .set(FeatureFlags::PRIVATE_SUBMISSION_REQUIRED);
        }
    }

    /// Analyze contract for gas costs and safety
    fn analyze_contract(
        mir: &MirModule,
        options: &CompilationOptions,
    ) -> CompilerResult<(Option<GasReport>, Option<VerificationReport>)> {
        let rules = SafetyRules::default();

        // Gas analysis
        let gas_report = if options.analyze_gas {
            let analyzer = GasAnalyzer::new(rules.clone());
            Some(analyzer.analyze(&mir.functions))
        } else {
            None
        };

        // Contract verification
        let verification_report = if options.verify_contract {
            let verifier = Verifier::new(rules);
            verifier.verify_mir(mir).ok()
        } else {
            None
        };

        Ok((gas_report, verification_report))
    }

    /// Run optimization pipeline on MIR
    fn optimize_mir(
        mir: &MirModule,
        options: &CompilationOptions,
    ) -> CompilerResult<(MirModule, Option<OptStats>)> {
        let mut optimized = mir.clone();

        if matches!(options.opt_level, OptLevel::None) {
            if options.verbose {
                eprintln!("    → No optimization (pass-through)");
            }
            return Ok((optimized, None));
        }

        let optimizer = Optimizer::new(options.opt_level);
        let stats = optimizer
            .run(&mut optimized)
            .map_err(|e| CompilerError::Optimization(format!("Optimization failed: {}", e)))?;

        if options.verbose {
            eprintln!("    📊 Optimization stats:");
            eprintln!("       • Passes executed: {}", stats.passes_run);
            eprintln!("       • Passes changed code: {}", stats.passes_changed);
            eprintln!(
                "       • Total transformations: {}",
                stats.total_transformations
            );
            eprintln!("       • Iterations to fixpoint: {}", stats.iterations);
        }

        Ok((optimized, Some(stats)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compilation_options_defaults() {
        let opts = CompilationOptions::default();
        assert!(!opts.debug);
        assert!(!opts.verbose);
    }

    #[test]
    fn test_compilation_options_builder() {
        let opts = CompilationOptions::opt2()
            .with_debug(true)
            .with_verbose(true);

        assert!(opts.debug);
        assert!(opts.verbose);
    }

    #[test]
    fn test_compile_simple_source() {
        let source = r#"
            fn main() -> i64 {
                let x = 10;
                let y = 20;
                return x + y;
            }
        "#;

        let options = CompilationOptions::opt2().with_verbose(false);
        let result = Compiler::compile(source, options);

        assert!(result.is_ok(), "Compilation should succeed");
        let output = result.unwrap();
        assert!(
            !output.bytecode.code.is_empty(),
            "Bytecode should not be empty"
        );
    }

    #[test]
    fn test_compile_with_artifacts() {
        let source = r#"
            fn add(a: i64, b: i64) -> i64 {
                return a + b;
            }

            fn main() -> i64 {
                return add(1, 2);
            }
        "#;

        let options = CompilationOptions::opt2()
            .with_emit_mir(true)
            .with_emit_stats(true);

        let result = Compiler::compile(source, options);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.artifacts.is_some());

        let artifacts = output.artifacts.unwrap();
        assert!(artifacts.mir_optimized.is_some());
        assert!(artifacts.opt_stats.is_some());
    }

    #[test]
    fn test_compile_with_gas_analysis() {
        let source = r#"
            fn compute(x: i64) -> i64 {
                let a = x * 2;
                let b = a + 10;
                return b;
            }

            fn main() -> i64 {
                return compute(4);
            }
        "#;

        let options = CompilationOptions::opt2().with_gas_analysis(true);

        let result = Compiler::compile(source, options);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.artifacts.is_some());

        let artifacts = output.artifacts.unwrap();
        assert!(artifacts.gas_report.is_some());

        let gas = artifacts.gas_report.unwrap();
        assert!(
            !gas.functions.is_empty(),
            "Should have at least one function"
        );
        assert!(gas.total_gas > 0, "Total gas should be positive");
    }

    #[test]
    fn test_contract_mode() {
        // Simple function without branches (backend doesn't handle conditionals well yet)
        let source = r#"
            fn compute(a: i64, b: i64) -> i64 {
                return a + b;
            }

            fn main() -> i64 {
                return compute(2, 3);
            }
        "#;

        let options = CompilationOptions::contract_mode();

        assert!(options.analyze_gas);
        assert!(options.verify_contract);

        let result = Compiler::compile(source, options);

        // Debug: print error if compilation fails
        if let Err(ref e) = result {
            eprintln!("Compilation error: {:?}", e);
        }

        assert!(result.is_ok(), "Compilation should succeed: {:?}", result);

        let output = result.unwrap();
        assert!(output.artifacts.is_some());

        let artifacts = output.artifacts.unwrap();
        assert!(artifacts.gas_report.is_some());
    }
}
