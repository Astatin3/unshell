//! # TCP Network Stack using Raw Syscalls
//!
//! A TCP server using raw syscalls via libc - no std::net, no smoltcp.
//!
//! ## Usage
//! ```bash
//! cargo run
//! nc 127.0.0.1 1337
//! ```

use libc::{accept, bind, listen, socket, sockaddr_in, socklen_t, read, write, close, AF_INET, SOCK_STREAM};
use core::mem::zeroed;

const PORT: u16 = 1337;
const BACKLOG: i32 = 128;

fn main() -> Result<(), NetError> {
    let server_fd = create_socket()?;
    bind_socket(server_fd, PORT)?;
    listen_socket(server_fd, BACKLOG)?;

    println!("TCP Server listening on port {}", PORT);
    println!("Connect with: nc 127.0.0.1 {}", PORT);

    let mut counter: u32 = 0;

    loop {
        match accept_client(server_fd) {
            Ok(client_fd) => {
                counter += 1;
                let message = make_packet(counter);

                unsafe {
                    let _ = write(client_fd, message.as_bytes().as_ptr() as *const libc::c_void, message.len());
                }

                let mut buf = [0u8; 1024];
                loop {
                    let n = unsafe {
                        read(client_fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len())
                    };
                    if n <= 0 {
                        break;
                    }
                }

                unsafe {
                    close(client_fd);
                }
            }
            Err(_) => continue,
        }
    }
}

fn create_socket() -> Result<i32, NetError> {
    let fd = unsafe {
        socket(AF_INET, SOCK_STREAM, 0)
    };
    if fd < 0 {
        return Err(NetError::DeviceError);
    }
    Ok(fd)
}

fn bind_socket(fd: i32, port: u16) -> Result<(), NetError> {
    let addr = build_sockaddr(port);
    let result = unsafe {
        bind(fd, &addr as *const sockaddr_in as *const libc::sockaddr, std::mem::size_of::<sockaddr_in>() as socklen_t)
    };
    if result < 0 {
        return Err(NetError::BindFailed);
    }
    Ok(())
}

fn listen_socket(fd: i32, backlog: i32) -> Result<(), NetError> {
    let result = unsafe { listen(fd, backlog) };
    if result < 0 {
        return Err(NetError::BindFailed);
    }
    Ok(())
}

fn accept_client(server_fd: i32) -> Result<i32, NetError> {
    let mut client_addr: sockaddr_in = unsafe { zeroed() };
    let mut client_len: socklen_t = std::mem::size_of::<sockaddr_in>() as socklen_t;

    let client_fd = unsafe {
        accept(server_fd, &mut client_addr as *mut sockaddr_in as *mut libc::sockaddr, &mut client_len)
    };

    if client_fd < 0 {
        return Err(NetError::NoConnection);
    }
    Ok(client_fd)
}

fn build_sockaddr(port: u16) -> sockaddr_in {
    sockaddr_in {
        sin_family: libc::AF_INET as u16,
        sin_port: port.to_be(),
        sin_addr: libc::in_addr {
            s_addr: u32::from_le_bytes([127, 0, 0, 1]),
        },
        sin_zero: [0; 8],
    }
}

#[derive(Debug, Clone, Copy)]
pub enum NetError {
    NoConnection,
    BindFailed,
    DeviceError,
}

impl core::fmt::Display for NetError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoConnection => write!(f, "No connection"),
            Self::BindFailed => write!(f, "Bind failed"),
            Self::DeviceError => write!(f, "Device error"),
        }
    }
}

fn make_packet(n: u32) -> SmallStr<16> {
    let mut result = SmallStr::<16>::new();
    result.push_slice(b"Packet #");
    push_number(&mut result, n);
    result.push(b'\n');
    result
}

fn push_number(s: &mut SmallStr<16>, n: u32) {
    if n == 0 {
        s.push(b'0');
        return;
    }
    let mut digits = [0u8; 10];
    let mut len = 0;
    let mut num = n;
    while num > 0 {
        digits[len] = b'0' + (num % 10) as u8;
        len += 1;
        num /= 10;
    }
    for i in (0..len).rev() {
        s.push(digits[i]);
    }
}

pub struct SmallStr<const N: usize> {
    data: [u8; N],
    len: usize,
}

impl<const N: usize> SmallStr<N> {
    pub const fn new() -> Self {
        Self { data: [0u8; N], len: 0 }
    }
    pub fn push(&mut self, byte: u8) {
        if self.len < N {
            self.data[self.len] = byte;
            self.len += 1;
        }
    }
    pub fn push_slice(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.push(byte);
        }
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.data[..self.len]
    }
    pub fn len(&self) -> usize {
        self.len
    }
}

impl<const N: usize> Default for SmallStr<N> {
    fn default() -> Self {
        Self::new()
    }
}