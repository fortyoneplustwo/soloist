use std::{
    ffi::c_void,
    io::{BufReader, Read, Write},
    net::TcpStream,
};

use crate::libc;

#[derive(Debug)]
enum Opcode {
    Continuation,
    Text,
    Binary,
    Control,
    Ping,
}

#[inline]
fn fin(val: &u8) -> bool {
    val & (1u8 << 7) != 0u8
}

#[inline]
fn opcode(val: &u8) -> Opcode {
    let op = val & (u8::MAX >> 4);
    match op {
        0x0 => Opcode::Continuation,
        0x1 => Opcode::Text,
        0x2 => Opcode::Binary,
        0x9 => Opcode::Ping,
        _ => Opcode::Control,
    }
}

#[inline]
fn mask(val: &u8) -> bool {
    val & (1u8 << 7) != 0u8
}

#[inline]
fn len(val: &u8) -> u8 {
    val & (u8::MAX >> 1)
}

#[derive(Debug)]
pub enum Error {
    InvalidOpcode,
    InvalidMask,
    ConnClosed,
    RecvErr(i32),
    SendErr(i32),
    Ping(String),
}

fn recv_into_buf(fd: i32, epollfd: i32, buf: &mut [u8]) -> Result<(), self::Error> {
    let mut nread = 0;
    let mut result;

    'recv: loop {
        // NOTE: Never, ever, EVER call recv() with size=0!
        if buf.is_empty() {
            return Ok(());
        }

        unsafe {
            result = libc::recv(
                fd,
                buf.as_mut_ptr().add(nread) as *mut c_void,
                buf.len() - nread,
                libc::MSG_DONTWAIT,
            );
        }

        match result {
            0 => Err(self::Error::ConnClosed),
            r if r < 0 => {
                let errno = std::io::Error::last_os_error()
                    .raw_os_error()
                    .unwrap();
                if errno != libc::EAGAIN || errno != libc::EWOULDBLOCK {
                    return Err(self::Error::RecvErr(errno));
                }
                let mut ready_buf = [epoll::Event::new(epoll::Events::EPOLLIN, 0)];
                '_epoll_wait: loop {
                    let Ok(nready) = epoll::wait(epollfd, -1, &mut ready_buf) else {
                        // WARN: panics here too
                        // maybe we were interrupted?
                        // handle EINTR
                        panic!("epoll::wait error");
                    };
                    if nready > 0 {
                        continue 'recv;
                    }
                }
            }
            r if r as usize == buf.len() - nread => {
                return Ok(());
            }
            _ => {
                nread += result as usize;
                continue 'recv;
            }
        }?;
    }
}

pub fn recv_msg(raw_fd: i32, epollfd: i32) -> Result<String, Error> {
    let mut msg_buf = String::new();
    let mut is_ping = false;

    'recv_next_frame: loop {
        // Read first 2 bytes containing: FIN, OPCODE, MASK, PAYLOAD_LEN
        let mut buf = [0u8; 2];
        recv_into_buf(raw_fd, epollfd, &mut buf)?;

        let is_fin = fin(&buf[0]);

        let should_skip = match opcode(&buf[0]) {
            Opcode::Text => false,
            Opcode::Continuation => msg_buf.is_empty(),
            Opcode::Ping => false,
            _ => true,
        };

        if let Opcode::Ping = opcode(&buf[0]) {
            is_ping = true;
        }

        if mask(&buf[1]) {
            panic!("Invalid mask");
        }

        // Read payload length
        let mut payload_len: u64 = len(&buf[1]).into();

        // Must re-read length from next 2 bytes
        if payload_len == 126 {
            let mut buf = [0u8; 2];
            recv_into_buf(raw_fd, epollfd, &mut buf)?;
            payload_len = u16::from_be_bytes(buf) as u64;
        }

        // Must re-read length from next 8 bytes
        if payload_len == 126 {
            let mut buf = [0u8; 8];
            recv_into_buf(raw_fd, epollfd, &mut buf)?;
            payload_len = u64::from_be_bytes(buf);
        }

        // Read the payload from the next payload_len bytes
        let mut buf = vec![0u8; payload_len as usize];
        recv_into_buf(raw_fd, epollfd, &mut buf)?;

        // Decide whether to append to our message or not
        if should_skip {
            break 'recv_next_frame;
        }
        msg_buf.push_str(String::from_utf8(buf).unwrap().as_str());

        // Break if msg is complete, otherwise loop back to recv next frame
        if is_fin {
            break 'recv_next_frame;
        }
    }

    match is_ping {
        true => Err(self::Error::Ping(msg_buf)),
        false => Ok(msg_buf),
    }
}

pub fn connect(addr: &str) -> Result<TcpStream, Box<dyn std::error::Error>> {
    // TODO: replace hardcoded key with nonce
    let _nonce = "dGhlIHNhbXBsZSBub25jZQ==";

    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .write_all(
            b"\
                GET / HTTP/1.1\r\n\
Host: localhost:9090\r\n\
Upgrade: websocket\r\n\
Connection: Upgrade\r\n\
Sec-WebSocket-Key: {_nonce}\r\n\
Sec-WebSocket-Version: 13\r\n\r\n\
                ",
        )
        .unwrap();

    let mut reader = BufReader::new(&stream);

    let mut line = String::new();
    let mut buf = [0u8; 1];

    reader.read_exact(&mut buf).unwrap();
    while buf[0] != b'\n' {
        if buf[0] != b'\r' {
            line.push(buf[0] as char);
        }
        reader.read_exact(&mut buf).unwrap();
    }

    if !line.starts_with("HTTP/1.1 101") {
        return Err("not ok".into());
    }

    // TODO: verify Sec-WebSocket-Accept here

    Ok(stream)
}

pub fn send_pong(raw_fd: i32, msg: &[u8]) -> Result<(), self::Error> {
    const FIN: u8 = 0b10000000;

    const RSV1: u8 = 0b00000000;
    const RSV2: u8 = 0b00000000;
    const RSV3: u8 = 0b00000000;

    const OPCODE: u8 = 0xA;

    const BYTE1: u8 = FIN | RSV1 | RSV2 | RSV3 | OPCODE;

    const MASK_FLAG: u8 = 0b10000000;

    const MASK_KEY: [u8; 4] = [1, 2, 3, 4];

    let mut payload_len: Vec<u8> = match msg.len() {
        x if x <= 125 => vec![x as u8],
        x if x <= u16::MAX as usize => {
            let mut v = vec![126u8];
            let length_as_bytes = (msg.len() as u16).to_be_bytes();
            v.extend_from_slice(&length_as_bytes);
            v
        }
        _ => {
            let mut v = vec![127u8];
            let length_as_bytes = (msg.len() as u64).to_be_bytes();
            v.extend_from_slice(&length_as_bytes);
            v
        }
    };

    let byte2 = MASK_FLAG | payload_len.remove(0);

    let mut frame = vec![BYTE1, byte2];
    frame.append(&mut payload_len);
    frame.extend_from_slice(&MASK_KEY);
    let mut masked_payload = msg
        .iter()
        .zip(MASK_KEY.iter().cycle())
        .map(|(a, b)| *a ^ b)
        .collect();
    frame.append(&mut masked_payload);

    let mut nwritten = 0;
    let mut result;

    'send: loop {
        unsafe {
            result = libc::send(
                raw_fd,
                frame.as_mut_ptr().add(nwritten) as *mut c_void,
                frame.len() - nwritten,
                0,
            );
        }

        match result {
            0 => Err(self::Error::ConnClosed),
            -1 => {
                let errno = std::io::Error::last_os_error()
                    .raw_os_error()
                    .unwrap();
                Err(self::Error::SendErr(errno))
            }
            r if r as usize == frame.len() - nwritten => {
                return Ok(());
            }
            _ => {
                nwritten += result as usize;
                continue 'send;
            }
        }?
    }
}

pub fn send_msg(raw_fd: i32, msg: &[u8]) -> Result<(), self::Error> {
    const FIN: u8 = 0b10000000;

    const RSV1: u8 = 0b00000000;
    const RSV2: u8 = 0b00000000;
    const RSV3: u8 = 0b00000000;

    const OPCODE: u8 = 0b00000001;

    const BYTE1: u8 = FIN | RSV1 | RSV2 | RSV3 | OPCODE;

    const MASK_FLAG: u8 = 0b10000000;

    const MASK_KEY: [u8; 4] = [1, 2, 3, 4];

    let mut payload_len: Vec<u8> = match msg.len() {
        x if x <= 125 => vec![x as u8],
        x if x <= u16::MAX as usize => {
            let mut v = vec![126u8];
            let length_as_bytes = (msg.len() as u16).to_be_bytes();
            v.extend_from_slice(&length_as_bytes);
            v
        }
        _ => {
            let mut v = vec![127u8];
            let length_as_bytes = (msg.len() as u64).to_be_bytes();
            v.extend_from_slice(&length_as_bytes);
            v
        }
    };

    let byte2 = MASK_FLAG | payload_len.remove(0);

    let mut frame = vec![BYTE1, byte2];
    frame.append(&mut payload_len);
    frame.extend_from_slice(&MASK_KEY);
    let mut masked_payload = msg
        .iter()
        .zip(MASK_KEY.iter().cycle())
        .map(|(a, b)| *a ^ b)
        .collect();
    frame.append(&mut masked_payload);

    let mut nwritten = 0;
    let mut result;

    'send: loop {
        unsafe {
            result = libc::send(
                raw_fd,
                frame.as_mut_ptr().add(nwritten) as *mut c_void,
                frame.len() - nwritten,
                0,
            );
        }

        match result {
            0 => Err(self::Error::ConnClosed),
            -1 => {
                let errno = std::io::Error::last_os_error()
                    .raw_os_error()
                    .unwrap();
                Err(self::Error::SendErr(errno))
            }
            r if r as usize == frame.len() - nwritten => {
                return Ok(());
            }
            _ => {
                nwritten += result as usize;
                continue 'send;
            }
        }?
    }
}
