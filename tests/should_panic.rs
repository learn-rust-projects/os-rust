// in tests/should_panic.rs

#![no_std]
#![no_main]

use core::panic::PanicInfo;

use os_rust::{QemuExitCode, exit_qemu, serial_println};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    should_fail();
    serial_println!("[test did not panic]");
    exit_qemu(QemuExitCode::Failed);
    #[allow(clippy::empty_loop)]
    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}

use os_rust::serial_print;

fn should_fail() {
    serial_print!("should_fail... ");
    assert_eq!(0, 1);
}
