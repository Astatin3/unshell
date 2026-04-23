//! # TCP Network Stack using Raw Syscalls
//!
//! A TCP server using raw x86/64 Linux syscalls via inline assembly - no libc, no std.
//!
//! ## Usage
//! ```bash
//! cargo run
//! nc 127.0.0.1 1337
//! ```

#![no_std]
#![no_main]

use core::arch::asm;

const PORT: u16 = 1337;
const BACKLOG: i32 = 128;

const AF_INET: i32 = 2;
const SOCK_STREAM: i32 = 1;
const IPPROTO_IP: i32 = 0;

const SYS_SOCKET: i32 = 41;
const SYS_BIND: i32 = 49;
const SYS_LISTEN: i32 = 50;
const SYS_ACCEPT: i32 = 43;
const SYS_WRITE: i32 = 1;
const SYS_CLOSE: i32 = 3;
const SYS_EXIT: i32 = 60;

#[repr(C)]
struct SockAddrIn {
    sin_family: u16,
    sin_port: u16,
    sin_addr: u32,
    sin_zero: [u8; 8],
}

#[repr(C)]
struct SockLen {
    len: u32,
}

impl SockLen {
    fn new() -> Self {
        Self { len: core::mem::size_of::<SockAddrIn>() as u32 }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() {
    log_info("starting tcp server");

    let server_fd = match create_socket() {
        Ok(fd) => {
            log_num("socket fd=", fd as i64);
            fd
        }
        Err(err) => {
            log_num("socket() failed errno=", err.errno as i64);
            exit_with(1)
        }
    };

    if let Err(err) = bind_socket(server_fd, PORT) {
        log_num("bind() failed errno=", err.errno as i64);
        exit_with(1);
    }
    log_info("bound to 127.0.0.1");

    if let Err(err) = listen_socket(server_fd, BACKLOG) {
        log_num("listen() failed errno=", err.errno as i64);
        exit_with(1);
    }

    log_info("socket is now listening");

    print_string("TCP Server listening on port ");
    print_u16(PORT);
    print_string("\n");

    let mut counter: u32 = 0;

    loop {
        match accept_client(server_fd) {
            Ok(client_fd) => {
                log_num("accepted client fd=", client_fd as i64);
                print_string("Connect with: nc 127.0.0.1 ");
                print_u16(PORT);
                print_string("\n");

                counter += 1;
                let message = make_packet(counter);
                let _ = syscall3(SYS_WRITE, client_fd as u64, message.as_ptr() as u64, message.len as u64);

                syscall1(SYS_CLOSE, client_fd as u64);
                print_string("Closed\n");
            }
            Err(err) => {
                log_num("accept() failed errno=", err.errno as i64);
                continue;
            }
        }
    }
}

fn syscall1(num: i32, arg1: u64) -> i64 {
    let result: i64;
    unsafe {
        asm!(
            "syscall",
            in("rax") num as u64,
            in("rdi") arg1,
            lateout("rax") result,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack)
        );
    }
    result
}

fn syscall3(num: i32, arg1: u64, arg2: u64, arg3: u64) -> i64 {
    let result: i64;
    unsafe {
        asm!(
            "syscall",
            in("rax") num as u64,
            in("rdi") arg1,
            in("rsi") arg2,
            in("rdx") arg3,
            lateout("rax") result,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack)
        );
    }
    result
}

fn syscall6(num: i32, arg1: u64, arg2: u64, arg3: u64, arg4: u64, arg5: u64, arg6: u64) -> i64 {
    let result: i64;
    unsafe {
        asm!(
            "syscall",
            in("rax") num as u64,
            in("rdi") arg1,
            in("rsi") arg2,
            in("rdx") arg3,
            in("r10") arg4,
            in("r8") arg5,
            in("r9") arg6,
            lateout("rax") result,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack)
        );
    }
    result
}

fn create_socket() -> Result<i32, SysErr> {
    let fd = syscall3(SYS_SOCKET, AF_INET as u64, SOCK_STREAM as u64, IPPROTO_IP as u64);
    if fd < 0 {
        return Err(SysErr::from_ret(fd));
    }
    Ok(fd as i32)
}

fn bind_socket(fd: i32, port: u16) -> Result<(), SysErr> {
    let addr = SockAddrIn {
        sin_family: AF_INET as u16,
        sin_port: port.to_be(),
        sin_addr: 0x0100007F,
        sin_zero: [0; 8],
    };

    let result = syscall6(
        SYS_BIND,
        fd as u64,
        (&addr as *const SockAddrIn) as u64,
        core::mem::size_of::<SockAddrIn>() as u64,
        0,
        0,
        0,
    );
    if result < 0 {
        return Err(SysErr::from_ret(result));
    }
    Ok(())
}

fn listen_socket(fd: i32, backlog: i32) -> Result<(), SysErr> {
    let result = syscall2(SYS_LISTEN, fd as u64, backlog as u64);
    if result < 0 {
        return Err(SysErr::from_ret(result));
    }
    Ok(())
}

fn syscall2(num: i32, arg1: u64, arg2: u64) -> i64 {
    let result: i64;
    unsafe {
        asm!(
            "syscall",
            in("rax") num as u64,
            in("rdi") arg1,
            in("rsi") arg2,
            lateout("rax") result,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack)
        );
    }
    result
}

fn accept_client(server_fd: i32) -> Result<i32, SysErr> {
    let mut addr: SockAddrIn = SockAddrIn {
        sin_family: 0,
        sin_port: 0,
        sin_addr: 0,
        sin_zero: [0; 8],
    };
    let mut addr_len: SockLen = SockLen::new();

    let client_fd = syscall6(
        SYS_ACCEPT,
        server_fd as u64,
        (&mut addr as *mut SockAddrIn) as u64,
        (&mut addr_len as *mut SockLen) as u64,
        0,
        0,
        0,
    );

    if client_fd < 0 {
        return Err(SysErr::from_ret(client_fd));
    }
    Ok(client_fd as i32)
}

#[derive(Clone, Copy)]
struct SysErr {
    errno: i32,
}

impl SysErr {
    fn from_ret(ret: i64) -> Self {
        Self { errno: (-ret) as i32 }
    }
}

fn exit_with(code: i32) -> ! {
    let _ = syscall1(SYS_EXIT, code as u64);
    loop {}
}

fn log_info(msg: &str) {
    write_stderr("[net] ");
    write_stderr(msg);
    write_stderr("\n");
}

fn log_num(prefix: &str, value: i64) {
    write_stderr("[net] ");
    write_stderr(prefix);
    print_i64_stderr(value);
    write_stderr("\n");
}

fn write_stderr(s: &str) {
    let _ = syscall3(SYS_WRITE, 2, s.as_ptr() as u64, s.len() as u64);
}

fn print_i64_stderr(n: i64) {
    if n < 0 {
        write_stderr("-");
    }
    print_u64_stderr(n.unsigned_abs());
}

fn print_u64_stderr(mut n: u64) {
    let mut buf = [0u8; 20];
    if n == 0 {
        buf[0] = b'0';
        let _ = syscall3(SYS_WRITE, 2, buf.as_ptr() as u64, 1);
        return;
    }

    let mut len = 0usize;
    while n > 0 {
        buf[len] = b'0' + (n % 10) as u8;
        len += 1;
        n /= 10;
    }
    let mut out = [0u8; 20];
    let mut i = 0usize;
    while i < len {
        out[i] = buf[len - 1 - i];
        i += 1;
    }
    let _ = syscall3(SYS_WRITE, 2, out.as_ptr() as u64, len as u64);
}

fn print_string(s: &str) {
    let stdout = 1u64;
    let buf_ptr = s.as_bytes().as_ptr();
    let len = s.len() as u64;
    let _ = syscall3(SYS_WRITE, stdout, buf_ptr as u64, len);
}

fn print_u16(n: u16) {
    let mut buf = [0u8; 6];
    let mut pos = 0;

    if n == 0 {
        buf[0] = b'0';
        pos = 1;
    } else {
        let mut digits = [0u8; 5];
        let mut count = 0;
        let mut num = n;
        while num > 0 {
            digits[count] = b'0' + (num % 10) as u8;
            count += 1;
            num /= 10;
        }
        let mut i = count;
        while i > 0 {
            i -= 1;
            buf[pos] = digits[i];
            pos += 1;
        }
    }

    let _ = syscall3(SYS_WRITE, 1u64, buf.as_ptr() as u64, pos as u64);
}

struct PacketBuf {
    data: [u8; 32],
    len: usize,
}

impl PacketBuf {
    fn new() -> Self {
        Self {
            data: [0u8; 32],
            len: 0,
        }
    }

    fn push(&mut self, byte: u8) {
        if self.len < 32 {
            self.data[self.len] = byte;
            self.len += 1;
        }
    }

    fn push_str(&mut self, s: &str) {
        for &b in s.as_bytes() {
            self.push(b);
        }
    }

    fn push_u32(&mut self, n: u32) {
        if n == 0 {
            self.push(b'0');
            return;
        }
        let mut digits = [0u8; 10];
        let mut count = 0;
        let mut num = n;
        while num > 0 {
            digits[count] = b'0' + (num % 10) as u8;
            count += 1;
            num /= 10;
        }
        for i in (0..count).rev() {
            self.push(digits[i]);
        }
    }

    fn as_ptr(&self) -> *const u8 {
        self.data.as_ptr()
    }
}

fn make_packet(n: u32) -> PacketBuf {
    let mut buf = PacketBuf::new();
    buf.push_str("Packet #");
    buf.push_u32(n);
    buf.push(b'\n');
    buf
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    log_info("panic");
    loop {}
}
