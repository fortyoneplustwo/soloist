use std::ffi::{c_int, c_void};

pub const MSG_DONTWAIT: c_int = 0x40;
pub const MSG_PEEK: c_int = 0x02;

pub const EAGAIN: c_int = 11;
pub const EWOULDBLOCK: c_int = EAGAIN;

unsafe extern "C" {
    pub unsafe fn recv(fd: c_int, buf: *mut c_void, size: usize, flags: c_int) -> isize;

    pub unsafe fn send(fd: c_int, buf: *mut c_void, size: usize, flags: c_int) -> isize;
}
