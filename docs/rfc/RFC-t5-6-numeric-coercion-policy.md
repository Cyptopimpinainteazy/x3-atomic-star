# RFC t5-6: Numeric Literal Coercion and Argument Type Error Policy

**Status:** ACCEPTED for X3Lang 1.0 baseline — **amended 2026-09-26** (literal typing; see "Amendment 1")
**Scope:** canonical Rust compiler path under `x3-lang/compiler`; root `crates/x3-typeck` remains a compatibility/integration implementation and must not define divergent language semantics
**Risk:** MEDIUM — affects language semantics and diagnostic consistency

---

## Amendment 1 (2026-09-26): an unsuffixed literal takes its context's integer type

Rules 1, 2 and the literal examples below are superseded by this amendment; rules 3–6 stand for
**typed** values.

- An unsuffixed integer literal takes the integer type its use requires — a parameter, a
  declared binding, a return type, or the other operand of an arithmetic or comparison
  operator — provided every value it stands for fits that type.
- `-n` (negation applied to a literal) is one literal with a negative value, so it satisfies
  signed types only.
- A literal no context constrains defaults to `i64`, the X3VM's integer.
- There is still no implicit conversion between typed integers: a `u64` variable does not satisfy
  an `i64` or `u32` parameter, and a `u32` does not widen to `u64`.

Why: the X3VM's one integer representation is `i64`, and every program the chain compiler,
both X3 engines and the live-node tests run is written `fn main() -> i64 { return 42; }`. Under
the original rule 1 each of those is ill-typed, so the chain compiler could not run a type checker
at all — and without one, bool arithmetic, wrong return and argument types, non-bool conditions
and functions that fall off their end compiled and executed. Literal-range-aware inference was
already listed below as future work; this adopts it, deterministically and without any coercion
of typed values. Decided by the project owner.

Required examples under the amendment:

| source | verdict |
|---|---|
| `fn f(x: i64) {}  f(1)` | accepted (`1` takes `i64`) |
| `fn f(x: u32) {}  f(1)` | accepted (`1` fits `u32`) |
| `fn f(x: u8) {}  f(300)` | `X3E0202` (does not fit) |
| `fn f(x: u64) {}  f(-1)` | `X3E0202` (negative) |
| `let v: u64 = 1; fn f(x: i64) {}  f(v)` | `X3E0202` (typed value, no coercion) |
| `let v: u32 = 1; fn f(x: u64) {}  f(v)` | `X3E0202` (no widening) |

Implemented in `x3-lang/compiler/src/numeric.rs` (tests `x3-lang/compiler/tests/test_numeric_policy.rs`)
and in the chain compiler's checker `crates/x3-typeck/src/checker.rs` (tests
`crates/x3-typeck/tests/golden.rs`, and `crates/x3-integration/tests/differential.rs` for programs
the chain compiler must refuse).

## Decision (original, 1.0 baseline)

X3Lang 1.0 uses a conservative, deterministic integer policy:

1. A bare non-negative integer literal is `u64`.
2. A negative integer such as `-42` remains represented in the AST as unary negation applied to a positive integer literal; for numeric compatibility, the resulting expression type is `i64` for the current bare-literal baseline.
3. Integer widths and signedness must match exactly at direct function-call boundaries.
4. There is no implicit widening, narrowing, or signed/unsigned integer coercion.
5. Direct function argument incompatibility reports the stable compiler diagnostic `X3E0202` (`ArgumentTypeMismatch`).
6. Explicit literal suffix/cast syntax is not part of the current source-language contract. AST suffix variants are reserved for future syntax/tooling and must not be used to imply currently unsupported source syntax.

## Rationale

- Exact compatibility keeps compilation deterministic and prevents hidden value-changing conversions.
- Bare literals have one predictable default (`u64`).
- Unary negation preserves the parser model instead of inventing a separate signed-literal token kind.
- Treating the resulting bare negative expression as `i64` gives X3Lang a usable signed baseline while retaining unary-negation AST semantics.
- A dedicated call-site diagnostic gives tooling a stable machine-readable signal independent of diagnostic wording.

## Required examples

These are the X3Lang 1.0 baseline behaviors:

```x3
fn takes_u64(x: u64) { }
fn main() { takes_u64(1); }
```

Accepted: bare `1` is `u64`.

```x3
fn takes_i64(x: i64) { }
fn main() { takes_i64(-1); }
```

Accepted: `-1` is unary negation and the resulting bare negative integer expression is `i64`.

```x3
fn takes_i64(x: i64) { }
fn main() { takes_i64(1); }
```

Rejected with `X3E0202`: no unsigned-to-signed coercion.

```x3
fn takes_u64(x: u64) { }
fn main() { takes_u64(-1); }
```

Rejected with `X3E0202`: no signed-to-unsigned coercion.

```x3
fn takes_u32(x: u32) { }
fn main() { takes_u32(1); }
```

Rejected with `X3E0202`: bare `1` is `u64`; implicit narrowing to `u32` is forbidden.

## Implementation authority

The accepted policy is implemented and regression-tested in the canonical Rust language workspace:

- `x3-lang/compiler/src/numeric.rs`
- `x3-lang/compiler/tests/test_numeric_policy.rs`
- stable codes in `x3-lang/compiler/src/diagnostic.rs`

The root workspace `crates/x3-parser` / `crates/x3-typeck` may temporarily retain older behavior while compatibility migration is in progress, but those crates do not supersede this policy. Differential tests should be used during migration where practical.

## Future work

Future RFCs may add:

- explicit integer literal suffix syntax;
- explicit cast syntax;
- checked widening conversions;
- literal-range-aware inference;
- richer first-class numeric types.

Any such change is a language-semantic change and requires explicit specification and conformance coverage. It must not silently alter the X3Lang 1.0 baseline.
