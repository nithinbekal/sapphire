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
fn test_underscore_in_integer() {
    assert_eq!(scan("1_000"), vec![TokenKind::Number(1000), TokenKind::Eof]);
    assert_eq!(
        scan("1_000_000"),
        vec![TokenKind::Number(1000000), TokenKind::Eof]
    );
}

#[test]
fn test_underscore_in_float() {
    assert_eq!(
        scan("1_000.5"),
        vec![TokenKind::Float(1000.5), TokenKind::Eof]
    );
    assert_eq!(
        scan("3.141_592"),
        vec![TokenKind::Float(3.141592), TokenKind::Eof]
    );
}

#[test]
fn test_hex_literal() {
    assert_eq!(scan("0xFF"), vec![TokenKind::Number(255), TokenKind::Eof]);
    assert_eq!(
        scan("0xFF_FF"),
        vec![TokenKind::Number(65535), TokenKind::Eof]
    );
    assert_eq!(
        scan("0xDEAD_BEEF"),
        vec![TokenKind::Number(3735928559), TokenKind::Eof]
    );
    assert_eq!(scan("0x0"), vec![TokenKind::Number(0), TokenKind::Eof]);
}

#[test]
fn test_heredoc_basic() {
    let src = "\"\"\"\n    hello\n    world\n    \"\"\"";
    assert_eq!(
        scan(src),
        vec![TokenKind::StringLit("hello\nworld".into()), TokenKind::Eof]
    );
}

#[test]
fn test_heredoc_no_indent() {
    let src = "\"\"\"\nhello\nworld\n\"\"\"";
    assert_eq!(
        scan(src),
        vec![TokenKind::StringLit("hello\nworld".into()), TokenKind::Eof]
    );
}

#[test]
fn test_heredoc_single_line() {
    let src = "\"\"\"\n    hello\n    \"\"\"";
    assert_eq!(
        scan(src),
        vec![TokenKind::StringLit("hello".into()), TokenKind::Eof]
    );
}

#[test]
fn test_heredoc_interpolation() {
    let src = "\"\"\"\n    hello #{name}\n    \"\"\"";
    assert_eq!(
        scan(src),
        vec![
            TokenKind::StringInterp(vec![
                ("hello ".into(), false),
                ("name".into(), true),
            ]),
            TokenKind::Eof
        ]
    );
}

#[test]
fn test_heredoc_empty_line_preserved() {
    let src = "\"\"\"\n    hello\n\n    world\n    \"\"\"";
    assert_eq!(
        scan(src),
        vec![TokenKind::StringLit("hello\n\nworld".into()), TokenKind::Eof]
    );
}

#[test]
fn test_heredoc_escape_sequences() {
    let src = "\"\"\"\n    tab:\\there\n    \"\"\"";
    assert_eq!(
        scan(src),
        vec![TokenKind::StringLit("tab:\there".into()), TokenKind::Eof]
    );
}

#[test]
fn test_heredoc_trailing_newline() {
    // Explicit trailing newline before closing """ is preserved
    let src = "\"\"\"\n    hello\n    \n    \"\"\"";
    assert_eq!(
        scan(src),
        vec![TokenKind::StringLit("hello\n".into()), TokenKind::Eof]
    );
}
