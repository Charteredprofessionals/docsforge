//! rules/parser.rs — Lexer + recursive-descent parser for the rules DSL (TASK-114, REQ-037).
//!
//! The grammar is intentionally tiny and closed: field references, literals
//! (number / text / boolean), comparison (`== != < <= > >=`), logical
//! (`&& || !`) and parentheses. There are no function calls, loops, or
//! assignments, so any string the parser accepts is inherently safe and
//! bounded (no Turing-complete surface).

use crate::core::error::DocForgeError;

/// A literal value in the DSL.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Number(f64),
    Text(String),
    Bool(bool),
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
}

/// A parsed expression tree.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    FieldRef(String),
    Literal(Literal),
    Binary(Box<Expr>, BinOp, Box<Expr>),
    Unary(UnaryOp, Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),
    Number(f64),
    Text(String),
    Bool(bool),
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Not,
    LParen,
    RParen,
}

/// Lexes the source into tokens. Rejects anything outside the allowed set.
fn lex(input: &str) -> Result<Vec<Token>, DocForgeError> {
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let mut tokens = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            '=' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::Eq);
                    i += 2;
                } else {
                    return Err(DocForgeError::InvalidInput(format!(
                        "Unexpected '=' at position {i}; use '==' for equality"
                    )));
                }
            }
            '!' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::Ne);
                    i += 2;
                } else {
                    tokens.push(Token::Not);
                    i += 1;
                }
            }
            '<' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::Le);
                    i += 2;
                } else {
                    tokens.push(Token::Lt);
                    i += 1;
                }
            }
            '>' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::Ge);
                    i += 2;
                } else {
                    tokens.push(Token::Gt);
                    i += 1;
                }
            }
            '&' => {
                if i + 1 < chars.len() && chars[i + 1] == '&' {
                    tokens.push(Token::And);
                    i += 2;
                } else {
                    return Err(DocForgeError::InvalidInput(format!(
                        "Unexpected '&' at position {i}; use '&&' for logical and"
                    )));
                }
            }
            '|' => {
                if i + 1 < chars.len() && chars[i + 1] == '|' {
                    tokens.push(Token::Or);
                    i += 2;
                } else {
                    return Err(DocForgeError::InvalidInput(format!(
                        "Unexpected '|' at position {i}; use '||' for logical or"
                    )));
                }
            }
            '"' => {
                i += 1;
                let mut s = String::new();
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        i += 1;
                        s.push(chars[i]);
                    } else {
                        s.push(chars[i]);
                    }
                    i += 1;
                }
                if i >= chars.len() {
                    return Err(DocForgeError::InvalidInput("Unterminated string literal".to_string()));
                }
                i += 1; // closing quote
                tokens.push(Token::Text(s));
            }
            _ if c.is_ascii_digit() || (c == '-' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit()) => {
                let start = i;
                i += 1;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let num: f64 = input[start..i]
                    .parse()
                    .map_err(|_| DocForgeError::InvalidInput(format!("Invalid number at position {start}")))?;
                tokens.push(Token::Number(num));
            }
            _ if c.is_alphabetic() || c == '_' => {
                let start = i;
                i += 1;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.') {
                    i += 1;
                }
                let word = &input[start..i];
                match word {
                    "true" => tokens.push(Token::Bool(true)),
                    "false" => tokens.push(Token::Bool(false)),
                    "&&" | "||" | "==" | "!=" | "<" | "<=" | ">" | ">=" => {
                        return Err(DocForgeError::InvalidInput(format!(
                            "Operator '{word}' must be surrounded by spaces"
                        )));
                    }
                    _ => tokens.push(Token::Ident(word.to_string())),
                }
            }
            _ => {
                return Err(DocForgeError::InvalidInput(format!(
                    "Unexpected character '{c}' at position {i}"
                )));
            }
        }
    }
    Ok(tokens)
}

/// Parses a token stream into an `Expr` using recursive descent with operator
/// precedence (|| < && < comparison < unary ! < primary).
pub fn parse(input: &str) -> Result<Expr, DocForgeError> {
    let tokens = lex(input)?;
    let mut pos = 0;
    let expr = parse_or(&tokens, &mut pos)?;
    if pos != tokens.len() {
        return Err(DocForgeError::InvalidInput(
            "Unexpected trailing tokens after expression".to_string(),
        ));
    }
    Ok(expr)
}

fn peek<'a>(tokens: &'a [Token], pos: &usize) -> Option<&'a Token> {
    tokens.get(*pos)
}

fn parse_or(tokens: &[Token], pos: &mut usize) -> Result<Expr, DocForgeError> {
    let mut left = parse_and(tokens, pos)?;
    while matches!(peek(tokens, pos), Some(Token::Or)) {
        *pos += 1;
        let right = parse_and(tokens, pos)?;
        left = Expr::Binary(Box::new(left), BinOp::Or, Box::new(right));
    }
    Ok(left)
}

fn parse_and(tokens: &[Token], pos: &mut usize) -> Result<Expr, DocForgeError> {
    let mut left = parse_comparison(tokens, pos)?;
    while matches!(peek(tokens, pos), Some(Token::And)) {
        *pos += 1;
        let right = parse_comparison(tokens, pos)?;
        left = Expr::Binary(Box::new(left), BinOp::And, Box::new(right));
    }
    Ok(left)
}

fn parse_comparison(tokens: &[Token], pos: &mut usize) -> Result<Expr, DocForgeError> {
    let left = parse_unary(tokens, pos)?;
    match peek(tokens, pos) {
        Some(Token::Eq) | Some(Token::Ne) | Some(Token::Lt) | Some(Token::Le)
        | Some(Token::Gt) | Some(Token::Ge) => {
            let op = match tokens[*pos] {
                Token::Eq => BinOp::Eq,
                Token::Ne => BinOp::Ne,
                Token::Lt => BinOp::Lt,
                Token::Le => BinOp::Le,
                Token::Gt => BinOp::Gt,
                Token::Ge => BinOp::Ge,
                _ => unreachable!(),
            };
            *pos += 1;
            let right = parse_unary(tokens, pos)?;
            Ok(Expr::Binary(Box::new(left), op, Box::new(right)))
        }
        _ => Ok(left),
    }
}

fn parse_unary(tokens: &[Token], pos: &mut usize) -> Result<Expr, DocForgeError> {
    if matches!(peek(tokens, pos), Some(Token::Not)) {
        *pos += 1;
        let operand = parse_unary(tokens, pos)?;
        return Ok(Expr::Unary(UnaryOp::Not, Box::new(operand)));
    }
    parse_primary(tokens, pos)
}

fn parse_primary(tokens: &[Token], pos: &mut usize) -> Result<Expr, DocForgeError> {
    let tok = peek(tokens, pos).ok_or_else(|| DocForgeError::InvalidInput("Unexpected end of expression".to_string()))?;
    match tok {
        Token::LParen => {
            *pos += 1;
            let expr = parse_or(tokens, pos)?;
            match peek(tokens, pos) {
                Some(Token::RParen) => {
                    *pos += 1;
                    Ok(expr)
                }
                _ => Err(DocForgeError::InvalidInput("Expected ')'".to_string())),
            }
        }
        Token::Number(n) => {
            let v = *n;
            *pos += 1;
            Ok(Expr::Literal(Literal::Number(v)))
        }
        Token::Text(s) => {
            let v = s.clone();
            *pos += 1;
            Ok(Expr::Literal(Literal::Text(v)))
        }
        Token::Bool(b) => {
            let v = *b;
            *pos += 1;
            Ok(Expr::Literal(Literal::Bool(v)))
        }
        Token::Ident(name) => {
            let v = name.clone();
            *pos += 1;
            Ok(Expr::FieldRef(v))
        }
        _ => Err(DocForgeError::InvalidInput(format!(
            "Unexpected token in primary position: {:?}",
            tok
        ))),
    }
}

/// Collects every field reference in an expression tree.
pub fn collect_field_refs(expr: &Expr) -> Vec<String> {
    let mut out = Vec::new();
    collect_field_refs_inner(expr, &mut out);
    out
}

fn collect_field_refs_inner(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::FieldRef(name) => out.push(name.clone()),
        Expr::Literal(_) => {}
        Expr::Unary(_, e) => collect_field_refs_inner(e, out),
        Expr::Binary(l, _, r) => {
            collect_field_refs_inner(l, out);
            collect_field_refs_inner(r, out);
        }
    }
}
