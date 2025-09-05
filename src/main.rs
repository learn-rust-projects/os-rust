#![no_std] // 不链接 Rust 标准库
#![no_main] // 禁用所有 Rust 层级的入口点
#![feature(custom_test_frameworks)]
#![test_runner(os_rust::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

use os_rust::println;

#[unsafe(no_mangle)] // 不重整函数名
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    os_rust::init(); // new

    x86_64::instructions::interrupts::int3(); // new

    #[cfg(test)]
    test_main();

    println!("It did not crash!");
    #[allow(clippy::empty_loop)]
    loop {}
}

/// 这个函数将在 panic 时被调用
#[cfg(not(test))] // new attribute
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

// our panic handler in test mode
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    os_rust::test_panic_handler(info)
}
