use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;

use crate::vm::{VmError, VmValue};

fn raise(msg: impl Into<String>) -> VmError {
    VmError::Raised(VmValue::Str(msg.into()))
}

pub fn socket_connect(host: &str, port: i64, _line: u32) -> Result<BufReader<TcpStream>, VmError> {
    TcpStream::connect(format!("{}:{}", host, port))
        .map(BufReader::new)
        .map_err(|e| raise(format!("Socket.connect: {}", e)))
}

pub fn socket_write(reader: &mut BufReader<TcpStream>, data: &str, _line: u32) -> Result<(), VmError> {
    reader
        .get_mut()
        .write_all(data.as_bytes())
        .map_err(|e| raise(format!("socket.write: {}", e)))
}

pub fn socket_read_line(reader: &mut BufReader<TcpStream>, _line: u32) -> Result<String, VmError> {
    let mut buf = String::new();
    reader
        .read_line(&mut buf)
        .map_err(|e| raise(format!("socket.read_line: {}", e)))?;
    if buf.ends_with('\n') {
        buf.pop();
        if buf.ends_with('\r') {
            buf.pop();
        }
    }
    Ok(buf)
}

pub fn socket_read_bytes(reader: &mut BufReader<TcpStream>, n: i64, _line: u32) -> Result<String, VmError> {
    let n = n.max(0) as usize;
    let mut buf = vec![0u8; n];
    reader
        .read_exact(&mut buf)
        .map_err(|e| raise(format!("socket.read_bytes: {}", e)))?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

pub fn socket_read_all(reader: &mut BufReader<TcpStream>, _line: u32) -> Result<String, VmError> {
    let mut buf = String::new();
    reader
        .read_to_string(&mut buf)
        .map_err(|e| raise(format!("socket.read_all: {}", e)))?;
    Ok(buf)
}

pub fn extract_fd(fields: &HashMap<String, VmValue>, line: u32) -> Result<i64, VmError> {
    match fields.get("fd") {
        Some(VmValue::Int(n)) => Ok(*n),
        _ => Err(VmError::TypeError {
            message: "socket instance has invalid fd".into(),
            line,
        }),
    }
}
