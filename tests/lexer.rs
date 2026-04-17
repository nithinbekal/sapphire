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
