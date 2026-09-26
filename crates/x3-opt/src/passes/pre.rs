//! Redundancy elimination over the dominator tree.
//!
//! An expression is **fully redundant** at a statement when an identical pure expression over the
//! same operands was computed at a point that dominates it: every path to the second computation
//! passed through the first, and SSA operands cannot have changed in between. The second statement
//! is removed and its uses read the first result.
//!
//! This is the safe subset of partial redundancy elimination, and it is deliberately the only part
//! implemented. The previous pass under this name (a Morel-Renvoise sketch) was wrong in three
//! ways, each measured by `crates/x3-integration/tests/differential.rs`:
//!
//! - its availability and anticipation maps were built over the **whole module** keyed by
//!   `MirBlockId`, and every function has a block 0, so one function's expressions were "available"
//!   in another — the reason a program that declared `main` first failed to compile at O2
//!   (`MIR value MirValue(1) not found in register map`);
//! - it prepended "hoisted" computations to the **start** of the entry block, before the operands
//!   the entry block itself defines, so `let a = 17; let b = 5; return (a+b) + (a+b)` read `a`
//!   before it was written;
//! - it hoisted computations out of conditional code into the entry block, which **speculates**
//!   them: `if b != 0 { return a / b }` would divide on the path that tested `b` and found zero.
//!
//! Removing a computation that a dominating one already made cannot do any of those: nothing is
//! inserted, nothing moves, and a division that trapped would have trapped at the first site. Truly
//! partial redundancies (computed on some paths only) are left alone.

use crate::cfg::Cfg;
use crate::pass::{Pass, PassResult};
use crate::value_numbering::CanonicalExpr;
use crate::OptResult;
use std::collections::{BTreeMap, BTreeSet};
use x3_mir::{MirBlockId, MirFunction, MirModule, MirRhs, MirStatement, MirTerminator, MirValue};

/// The identity of a pure expression: its canonical form (commutative operands sorted) and, for a
/// binary operation, whether it is the float or the integer instruction.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExprKey {
    canonical: CanonicalExpr,
    float: bool,
}

impl ExprKey {
    /// The key of `rhs` if it is an expression this pass may deduplicate: a unary or binary
    /// operation. Literals, calls, loads and stores are not — a call may have effects and a load
    /// reads storage a store can change.
    pub fn from_rhs(rhs: &MirRhs) -> Option<Self> {
        match rhs {
            MirRhs::Binary {
                op,
                left,
                right,
                float,
            } => Some(ExprKey {
                canonical: CanonicalExpr::from_binary(*op, *left, *right),
                float: *float,
            }),
            MirRhs::Unary(op, operand) => Some(ExprKey {
                canonical: CanonicalExpr::from_unary(*op, *operand),
                float: false,
            }),
            _ => None,
        }
    }
}

/// Dominator-based redundancy elimination (see the module documentation).
///
/// The pass keeps its historical name so pipelines and telemetry that name it stay valid.
#[derive(Default)]
pub struct PrePass;

impl PrePass {
    pub fn new() -> Self {
        PrePass
    }

    /// Remove every fully redundant expression in `func`; returns how many were removed.
    pub fn eliminate_in_function(func: &mut MirFunction) -> usize {
        if func.blocks.is_empty() {
            return 0;
        }

        // A value that is the address of a register store is a mutable variable's storage, not an
        // SSA value: two reads of it may differ. Expressions are never built on one directly (reads
        // go through a `Load`), but an expression that were would not be pure in the sense this pass
        // needs, so it is skipped rather than assumed away.
        let cells: BTreeSet<MirValue> = func
            .blocks
            .iter()
            .flat_map(|block| block.statements.iter())
            .filter_map(|stmt| match stmt.rhs() {
                Some(MirRhs::Store { addr, .. }) => Some(*addr),
                _ => None,
            })
            .collect();

        let cfg = Cfg::from_function(func);
        let (idom, _) = cfg.compute_dominators();

        // Every candidate in block order, statement order: (block, index, key, target).
        let mut candidates: Vec<(MirBlockId, usize, ExprKey, MirValue)> = Vec::new();
        for block in &func.blocks {
            for (index, stmt) in block.statements.iter().enumerate() {
                let (Some(target), Some(rhs)) = (stmt.target(), stmt.rhs()) else {
                    continue;
                };
                if operands(rhs).iter().any(|operand| cells.contains(operand)) {
                    continue;
                }
                if let Some(key) = ExprKey::from_rhs(rhs) {
                    candidates.push((block.id, index, key, target));
                }
            }
        }

        // For each candidate, a surviving identical expression that dominates it.
        let mut replacements: BTreeMap<MirValue, MirValue> = BTreeMap::new();
        let mut removals: BTreeMap<MirBlockId, BTreeSet<usize>> = BTreeMap::new();
        for (i, (block, index, key, target)) in candidates.iter().enumerate() {
            let dominating = candidates
                .iter()
                .enumerate()
                .find(|(j, (other_block, other_index, other_key, other_target))| {
                    *j != i
                        && other_key == key
                        && !replacements.contains_key(other_target)
                        && if other_block == block {
                            other_index < index
                        } else {
                            cfg.dominates(*other_block, *block, &idom)
                        }
                })
                .map(|(_, (_, _, _, earlier))| *earlier);
            if let Some(earlier) = dominating {
                replacements.insert(*target, earlier);
                removals.entry(*block).or_default().insert(*index);
            }
        }

        if replacements.is_empty() {
            return 0;
        }

        for block in &mut func.blocks {
            if let Some(indices) = removals.get(&block.id) {
                let mut index = 0;
                block.statements.retain(|_| {
                    let keep = !indices.contains(&index);
                    index += 1;
                    keep
                });
            }
            for stmt in &mut block.statements {
                if let MirStatement::Assign { rhs, .. } = stmt {
                    replace_operands(rhs, &replacements);
                }
            }
            if let Some(term) = &mut block.terminator {
                replace_in_terminator(term, &replacements);
            }
        }

        replacements.len()
    }
}

impl Pass for PrePass {
    fn name(&self) -> &'static str {
        "partial_redundancy_elimination"
    }

    fn run(&self, module: &mut MirModule) -> OptResult<PassResult> {
        // Per function: value and block ids are local to a function, so nothing may be shared
        // between two of them.
        let removed: usize = module
            .functions
            .iter_mut()
            .map(Self::eliminate_in_function)
            .sum();
        Ok(PassResult::with_count(
            removed,
            "Removed fully redundant expressions",
        ))
    }
}

/// The values an expression reads.
fn operands(rhs: &MirRhs) -> Vec<MirValue> {
    match rhs {
        MirRhs::Literal(_) => vec![],
        MirRhs::Unary(_, v) => vec![*v],
        MirRhs::Binary { left, right, .. } => vec![*left, *right],
        MirRhs::Call { args, .. } => args.clone(),
        MirRhs::Load { addr, .. } => vec![*addr],
        MirRhs::Store { addr, val, .. } => vec![*addr, *val],
    }
}

/// Follow `replacements` to the surviving value.
fn resolve(value: MirValue, replacements: &BTreeMap<MirValue, MirValue>) -> MirValue {
    let mut current = value;
    for _ in 0..=replacements.len() {
        match replacements.get(&current) {
            Some(&next) if next != current => current = next,
            _ => break,
        }
    }
    current
}

fn replace_operands(rhs: &mut MirRhs, replacements: &BTreeMap<MirValue, MirValue>) {
    let fix = |v: &mut MirValue| *v = resolve(*v, replacements);
    match rhs {
        MirRhs::Literal(_) => {}
        MirRhs::Unary(_, v) => fix(v),
        MirRhs::Binary { left, right, .. } => {
            fix(left);
            fix(right);
        }
        MirRhs::Call { args, .. } => args.iter_mut().for_each(fix),
        MirRhs::Load { addr, .. } => fix(addr),
        MirRhs::Store { addr, val, .. } => {
            fix(addr);
            fix(val);
        }
    }
}

fn replace_in_terminator(term: &mut MirTerminator, replacements: &BTreeMap<MirValue, MirValue>) {
    match term {
        MirTerminator::Return(Some(v)) => *v = resolve(*v, replacements),
        MirTerminator::Branch { cond, .. } => *cond = resolve(*cond, replacements),
        MirTerminator::Return(None) | MirTerminator::Goto(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use x3_ast::{BinaryOp, UnaryOp};
    use x3_common::{Literal, Span};
    use x3_hir::hir::SymbolId;
    use x3_mir::MirBlock;

    fn lit(target: usize, n: i64) -> MirStatement {
        MirStatement::Assign {
            target: MirValue(target),
            rhs: MirRhs::Literal(Literal::Integer(n)),
        }
    }

    fn bin(target: usize, op: BinaryOp, l: usize, r: usize) -> MirStatement {
        MirStatement::Assign {
            target: MirValue(target),
            rhs: MirRhs::Binary {
                op,
                left: MirValue(l),
                right: MirValue(r),
                float: false,
            },
        }
    }

    fn block(id: usize, statements: Vec<MirStatement>, terminator: MirTerminator) -> MirBlock {
        MirBlock {
            id: MirBlockId(id),
            statements,
            terminator: Some(terminator),
        }
    }

    fn function(blocks: Vec<MirBlock>) -> MirFunction {
        MirFunction {
            symbol: SymbolId(0),
            params: vec![],
            entry: MirBlockId(0),
            blocks,
            span: Span::dummy(),
        }
    }

    fn module(functions: Vec<MirFunction>) -> MirModule {
        MirModule {
            functions,
            span: Span::dummy(),
        }
    }

    #[test]
    fn pass_name_is_stable() {
        assert_eq!(PrePass::new().name(), "partial_redundancy_elimination");
    }

    #[test]
    fn a_repeated_expression_in_one_block_reuses_the_first() {
        let mut m = module(vec![function(vec![block(
            0,
            vec![
                lit(0, 17),
                lit(1, 5),
                bin(2, BinaryOp::Add, 0, 1),
                bin(3, BinaryOp::Add, 1, 0), // commutative: the same expression
                bin(4, BinaryOp::Mul, 2, 3),
            ],
            MirTerminator::Return(Some(MirValue(4))),
        )])]);
        let result = PrePass::new().run(&mut m).unwrap();
        assert_eq!(result.transformations, 1);
        let stmts = &m.functions[0].blocks[0].statements;
        assert_eq!(
            stmts.len(),
            4,
            "the duplicate is removed, nothing is inserted"
        );
        // Definitions stay in order: the operands come before the expression that reads them.
        assert_eq!(stmts[0].target(), Some(MirValue(0)));
        assert_eq!(stmts[1].target(), Some(MirValue(1)));
        assert_eq!(
            stmts[3].rhs(),
            Some(&MirRhs::Binary {
                op: BinaryOp::Mul,
                left: MirValue(2),
                right: MirValue(2),
                float: false,
            })
        );
    }

    #[test]
    fn a_non_commutative_expression_with_swapped_operands_is_kept() {
        let mut m = module(vec![function(vec![block(
            0,
            vec![
                lit(0, 17),
                lit(1, 5),
                bin(2, BinaryOp::Sub, 0, 1),
                bin(3, BinaryOp::Sub, 1, 0),
            ],
            MirTerminator::Return(Some(MirValue(3))),
        )])]);
        let result = PrePass::new().run(&mut m).unwrap();
        assert!(!result.changed);
    }

    /// A division under a guard must not move above the guard, and must not be replaced by one
    /// that did not dominate it.
    #[test]
    fn an_expression_in_one_branch_is_not_used_by_the_other() {
        let mut m = module(vec![function(vec![
            block(
                0,
                vec![lit(0, 1), lit(1, 10), lit(2, 0)],
                MirTerminator::Branch {
                    cond: MirValue(0),
                    then_block: MirBlockId(1),
                    else_block: MirBlockId(2),
                },
            ),
            block(
                1,
                vec![bin(3, BinaryOp::Div, 1, 2)],
                MirTerminator::Return(Some(MirValue(3))),
            ),
            block(
                2,
                vec![bin(4, BinaryOp::Div, 1, 2)],
                MirTerminator::Return(Some(MirValue(4))),
            ),
        ])]);
        let result = PrePass::new().run(&mut m).unwrap();
        assert!(!result.changed, "neither branch dominates the other");
        assert_eq!(
            m.functions[0].blocks[0].statements.len(),
            3,
            "nothing hoisted"
        );
    }

    #[test]
    fn a_dominating_computation_serves_a_later_block() {
        let mut m = module(vec![function(vec![
            block(
                0,
                vec![lit(0, 3), lit(1, 4), bin(2, BinaryOp::Mul, 0, 1)],
                MirTerminator::Goto(MirBlockId(1)),
            ),
            block(
                1,
                vec![bin(3, BinaryOp::Mul, 0, 1)],
                MirTerminator::Return(Some(MirValue(3))),
            ),
        ])]);
        let result = PrePass::new().run(&mut m).unwrap();
        assert_eq!(result.transformations, 1);
        assert!(m.functions[0].blocks[1].statements.is_empty());
        assert_eq!(
            m.functions[0].blocks[1].terminator,
            Some(MirTerminator::Return(Some(MirValue(2))))
        );
    }

    /// Two functions share value and block ids; the pass may not treat one's expression as
    /// available in the other.
    #[test]
    fn functions_do_not_share_expressions() {
        let f = || {
            function(vec![block(
                0,
                vec![lit(0, 2), lit(1, 3), bin(2, BinaryOp::Add, 0, 1)],
                MirTerminator::Return(Some(MirValue(2))),
            )])
        };
        let mut m = module(vec![f(), f()]);
        let result = PrePass::new().run(&mut m).unwrap();
        assert!(!result.changed);
        assert_eq!(m.functions[1].blocks[0].statements.len(), 3);
    }

    #[test]
    fn a_unary_expression_is_deduplicated() {
        let mut m = module(vec![function(vec![block(
            0,
            vec![
                lit(0, 9),
                MirStatement::Assign {
                    target: MirValue(1),
                    rhs: MirRhs::Unary(UnaryOp::Negate, MirValue(0)),
                },
                MirStatement::Assign {
                    target: MirValue(2),
                    rhs: MirRhs::Unary(UnaryOp::Negate, MirValue(0)),
                },
                bin(3, BinaryOp::Add, 1, 2),
            ],
            MirTerminator::Return(Some(MirValue(3))),
        )])]);
        let result = PrePass::new().run(&mut m).unwrap();
        assert_eq!(result.transformations, 1);
    }

    #[test]
    fn three_identical_expressions_collapse_onto_the_first_deterministically() {
        let build = || {
            module(vec![function(vec![block(
                0,
                vec![
                    lit(0, 1),
                    lit(1, 2),
                    bin(2, BinaryOp::Add, 0, 1),
                    bin(3, BinaryOp::Add, 0, 1),
                    bin(4, BinaryOp::Add, 0, 1),
                    bin(5, BinaryOp::Add, 3, 4),
                ],
                MirTerminator::Return(Some(MirValue(5))),
            )])])
        };
        let (mut a, mut b) = (build(), build());
        PrePass::new().run(&mut a).unwrap();
        PrePass::new().run(&mut b).unwrap();
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
        assert_eq!(
            a.functions[0].blocks[0].statements[3].rhs(),
            Some(&MirRhs::Binary {
                op: BinaryOp::Add,
                left: MirValue(2),
                right: MirValue(2),
                float: false,
            })
        );
    }
}
