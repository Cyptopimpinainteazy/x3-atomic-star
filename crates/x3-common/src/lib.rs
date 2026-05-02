#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::string::String;
use core::{fmt, ops::Add, str::FromStr};

// Shared building blocks for the X3 compiler pipeline.

/// A byte index span that locates tokens and AST nodes inside source text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Create a dummy span for testing purposes.
    pub const fn dummy() -> Self {
        Self { start: 0, end: 0 }
    }

    pub fn merge(self, other: Span) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

impl Add for Span {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        self.merge(rhs)
    }
}

/// Literals that can appear inside the language.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Literal {
    Integer(i64),
    Float(f64),
    String(String),
    Bool(bool),
    /// Unit value - represents absence of meaningful value (like void/()).
    Unit,
}

/// Keywords recognized by the lexer and parser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keyword {
    Fn,
    Let,
    Mut,
    If,
    Else,
    While,
    Loop,
    For,
    Return,
    Break,
    Continue,
    Struct,
    Enum,
    Match,
    True,
    False,
    Atomic,
    Emit,
    Agent,
    Context,
    Const,
    In,
}

impl Keyword {
    pub fn parse(src: &str) -> Option<Self> {
        match src {
            "fn" => Some(Self::Fn),
            "let" => Some(Self::Let),
            "mut" => Some(Self::Mut),
            "if" => Some(Self::If),
            "else" => Some(Self::Else),
            "while" => Some(Self::While),
            "loop" => Some(Self::Loop),
            "for" => Some(Self::For),
            "return" => Some(Self::Return),
            "break" => Some(Self::Break),
            "continue" => Some(Self::Continue),
            "struct" => Some(Self::Struct),
            "enum" => Some(Self::Enum),
            "match" => Some(Self::Match),
            "true" => Some(Self::True),
            "false" => Some(Self::False),
            "atomic" => Some(Self::Atomic),
            "emit" => Some(Self::Emit),
            "agent" => Some(Self::Agent),
            "context" => Some(Self::Context),
            "const" => Some(Self::Const),
            "in" => Some(Self::In),
            _ => None,
        }
    }
}

/// Error returned when a keyword does not match a known value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeywordParseError;

impl fmt::Display for KeywordParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid keyword")
    }
}

impl core::error::Error for KeywordParseError {}

impl FromStr for Keyword {
    type Err = KeywordParseError;

    fn from_str(src: &str) -> Result<Self, Self::Err> {
        Keyword::parse(src).ok_or(KeywordParseError)
    }
}

/// Symbols used for delimiters and operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symbol {
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    Equals,
    DoubleEquals,
    Bang,
    BangEquals,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Amp,
    Pipe,
    Or,
    Arrow,
    FatArrow,
    Colon,
    Comma,
    Dot,
    Semicolon,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
}

/// Token kinds produced by the lexer.
#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Identifier(String),
    Keyword(Keyword),
    Symbol(Symbol),
    Literal(Literal),
    Eof,
}

impl TokenKind {
    pub fn symbol(&self) -> Option<Symbol> {
        match self {
            TokenKind::Symbol(sym) => Some(*sym),
            _ => None,
        }
    }
}

/// A token plus its span.
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}
