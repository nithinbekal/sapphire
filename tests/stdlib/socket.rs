use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use super::{eval, eval_err};
use sapphire::vm::{VmError, VmValue};

fn with_server<F>(handler: impl Fn(std::net::TcpStream) + Send + 'static, f: F)
where
    F: FnOnce(u16),
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            handler(stream);
        }
    });
    f(port);
}

#[test]
fn connect_write_read_all() {
    with_server(
        |mut stream| {
            let mut buf = vec![0u8; 64];
            let n = stream.read(&mut buf).unwrap_or(0);
            stream.write_all(&buf[..n]).ok();
        },
        |port| {
            let src = format!(
                r#"
                s = Socket.connect("127.0.0.1", {port})
                s.write("hello")
                s.read_all()
                "#
            );
            assert_eq!(eval(&src), VmValue::Str("hello".to_string()));
        },
    );
}

#[test]
fn read_line_strips_crlf() {
    with_server(
        |mut stream| {
            stream.write_all(b"HTTP/1.0 200 OK\r\n").ok();
        },
        |port| {
            let src = format!(
                r#"Socket.connect("127.0.0.1", {port}).read_line()"#
            );
            assert_eq!(eval(&src), VmValue::Str("HTTP/1.0 200 OK".to_string()));
        },
    );
}

#[test]
fn read_bytes_exact() {
    with_server(
        |mut stream| {
            stream.write_all(b"Hello, World!").ok();
        },
        |port| {
            let src = format!(
                r#"
                s = Socket.connect("127.0.0.1", {port})
                s.read_bytes(5)
                "#
            );
            assert_eq!(eval(&src), VmValue::Str("Hello".to_string()));
        },
    );
}

#[test]
fn close_removes_fd() {
    with_server(
        |mut stream| {
            stream.write_all(b"hi").ok();
        },
        |port| {
            let src = format!(
                r#"
                s = Socket.connect("127.0.0.1", {port})
                s.close()
                s.read_all()
                "#
            );
            assert!(matches!(eval_err(&src), VmError::Raised(_)));
        },
    );
}

#[test]
fn bad_host_raises() {
    let src = r#"Socket.connect("this.host.does.not.exist.invalid", 80)"#;
    assert!(matches!(eval_err(src), VmError::Raised(_)));
}
