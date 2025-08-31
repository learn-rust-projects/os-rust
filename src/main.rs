#![no_std] // 不链接 Rust 标准库
#![no_main] // 禁用所有 Rust 层级的入口点
mod vga_buffer;
use core::panic::PanicInfo;
#[unsafe(no_mangle)] // 不重整函数名
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");
    println!("Hello World{}", "!");
    #[allow(clippy::empty_loop)]
    loop {}
}

/// 这个函数将在 panic 时被调用
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}
