#![no_std]
#![no_main]

enum TestEnum {
    Test = 135,
}

#[unsafe(no_mangle)]
pub fn test() -> i32 {
    let a = TestEnum::Test;

    a as i32
}

#[unsafe(no_mangle)]
fn main() {
    let a = 5;

    unshell_obfuscate::junk_asm!(0.1);

    unsafe { libc::exit(a as i32) }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
