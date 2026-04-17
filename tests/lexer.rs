use sapphire::lexer::Lexer;
use sapphire::token::TokenKind;

fn scan(source: &str) -> Vec<TokenKind> {
    Lexer::new(source)
        .scan_tokens()
        .into_iter()
        .map(|t| t.kind)
        .collect()
}

#[test]
fn test_single_operators() {
    assert_eq!(scan("+"), vec![TokenKind::Plus, TokenKind::Eof]);
    assert_eq!(scan("-"), vec![TokenKind::Minus, TokenKind::Eof]);
    assert_eq!(scan("*"), vec![TokenKind::Star, TokenKind::Eof]);
    assert_eq!(scan("/"), vec![TokenKind::Slash, TokenKind::Eof]);
}

#[test]
fn test_sequence() {
    assert_eq!(
        scan("+-*/"),
        vec![
            TokenKind::Plus,
            TokenKind::Minus,
            TokenKind::Star,
            TokenKind::Slash,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn test_ignores_unknown_chars() {
    assert_eq!(scan("  "), vec![TokenKind::Eof]);
    assert_eq!(scan("@"), vec![TokenKind::Eof]);
}

#[test]
fn test_comments() {
    assert_eq!(scan("# hello"), vec![TokenKind::Eof]);
    assert_eq!(
        scan("1 # comment\n2"),
        vec![TokenKind::Number(1), TokenKind::Newline, TokenKind::Number(2), TokenKind::Eof]
    );
}

#[test]
fn test_empty() {
    assert_eq!(scan(""), vec![TokenKind::Eof]);
}

#[test]
fn test_integer() {
    assert_eq!(scan("42"), vec![TokenKind::Number(42), TokenKind::Eof]);
    assert_eq!(scan("0"), vec![TokenKind::Number(0), TokenKind::Eof]);
}

#[test]
fn test_integer_in_expression() {
    assert_eq!(
        scan("1+2"),
        vec![
            TokenKind::Number(1),
            TokenKind::Plus,
            TokenKind::Number(2),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn test_identifier() {
    assert_eq!(
        scan("foo"),
        vec![TokenKind::Identifier("foo".into()), TokenKind::Eof]
    );
    assert_eq!(
        scan("my_var"),
        vec![TokenKind::Identifier("my_var".into()), TokenKind::Eof]
    );
    assert_eq!(
        scan("empty?"),
        vec![TokenKind::Identifier("empty?".into()), TokenKind::Eof]
    );
}

#[test]
fn test_float_literal() {
    assert_eq!(scan("3.14"), vec![TokenKind::Float(3.14), TokenKind::Eof]);
    assert_eq!(scan("1.0"), vec![TokenKind::Float(1.0), TokenKind::Eof]);
}

#[test]
fn test_integer_dot_dot_not_float() {
    // `1..10` must lex as Number(1) DotDot Number(10), not as a float
    assert_eq!(
        scan("1..10"),
        vec![TokenKind::Number(1), TokenKind::DotDot, TokenKind::Number(10), TokenKind::Eof]
    );
}

#[test]
fn test_string_escape_sequences() {
    assert_eq!(scan(r#""\n""#), vec![TokenKind::StringLit("\n".into()), TokenKind::Eof]);
    assert_eq!(scan(r#""\t""#), vec![TokenKind::StringLit("\t".into()), TokenKind::Eof]);
    assert_eq!(scan(r#""\r""#), vec![TokenKind::StringLit("\r".into()), TokenKind::Eof]);
    assert_eq!(scan(r#""\\""#), vec![TokenKind::StringLit("\\".into()), TokenKind::Eof]);
    assert_eq!(scan(r#""\"""#), vec![TokenKind::StringLit("\"".into()), TokenKind::Eof]);
    assert_eq!(scan(r#""\#""#), vec![TokenKind::StringLit("#".into()), TokenKind::Eof]);
}

#[test]
fn test_string_escape_unknown_passthrough() {
    // Unknown escape sequences pass through both characters
    assert_eq!(scan(r#""\z""#), vec![TokenKind::StringLit("\\z".into()), TokenKind::Eof]);
}

#[test]
fn test_underscore_in_integer() {
    assert_eq!(scan("1_000"), vec![TokenKind::Number(1000), TokenKind::Eof]);
    assert_eq!(scan("1_000_000"), vec![TokenKind::Number(1000000), TokenKind::Eof]);
}

#[test]
fn test_underscore_in_float() {
    assert_eq!(scan("1_000.5"), vec![TokenKind::Float(1000.5), TokenKind::Eof]);
    assert_eq!(scan("3.141_592"), vec![TokenKind::Float(3.141592), TokenKind::Eof]);
}

#[test]
fn test_hex_literal() {
    assert_eq!(scan("0xFF"), vec![TokenKind::Number(255), TokenKind::Eof]);
    assert_eq!(scan("0xFF_FF"), vec![TokenKind::Number(65535), TokenKind::Eof]);
    assert_eq!(scan("0xDEAD_BEEF"), vec![TokenKind::Number(3735928559), TokenKind::Eof]);
    assert_eq!(scan("0x0"), vec![TokenKind::Number(0), TokenKind::Eof]);
}
