#![no_std]
#![no_main]

#[unsafe(no_mangle)]
fn main() {
    let a = 135;

    unshell_obfuscate::junk_asm!(15.);

    unsafe { libc::exit(a as i32) }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
