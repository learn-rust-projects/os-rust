#![no_std] // 不链接 Rust 标准库
#![no_main] // 禁用所有 Rust 层级的入口点
#![feature(custom_test_frameworks)]
#![test_runner(os_rust::test_runner)]
#![reexport_test_harness_main = "test_main"]
extern crate alloc;
use alloc::{boxed::Box, rc::Rc, vec, vec::Vec};
use core::panic::PanicInfo;

use bootloader::{BootInfo, entry_point};
use os_rust::println;

entry_point!(kernel_main);

/// 内核的主入口点函数
///
/// 这个函数在操作系统启动时由 bootloader 调用，是操作系统内核的主要执行起点。
/// 函数接收 boot_info 参数，其中包含了系统启动时的重要信息，如物理内存映射、
/// 内存偏移量等。
///
/// # 参数
/// - `boot_info`: 引导信息，包含系统启动时的关键信息
///
/// # 返回值
/// 函数返回 `!` 类型，表示这是一个永远不会返回的函数（发散函数）
fn kernel_main(boot_info: &'static BootInfo) -> ! {
    // 导入必要的函数和类型
    // active_level_4_table: 获取当前活动的 Level 4 页表的函数
    // VirtAddr: 表示虚拟内存地址的类型
    use os_rust::{allocator, memory};
    use x86_64::{
        VirtAddr,
        structures::paging::{Page, Translate},
    };

    // 在屏幕上打印 "Hello World!" 消息，作为内核启动的标志
    println!("Hello World{}", "!");

    // 初始化操作系统的关键组件，如 GDT（全局描述符表）、IDT（中断描述符表）等
    os_rust::init(); // new

    // 从引导信息中获取物理内存偏移量
    // 这个偏移量用于在虚拟地址空间中访问物理内存
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mapper = unsafe { memory::init(phys_mem_offset) };

    let addresses = [
        // the identity-mapped vga buffer page
        0xb8000,
        // some code page
        0x201008,
        // some stack page
        0x0100_0020_1a10,
        // virtual address mapped to physical address 0
        boot_info.physical_memory_offset,
    ];

    for &address in &addresses {
        let virt = VirtAddr::new(address);
        let phys = mapper.translate_addr(virt);
        println!("{:?} -> {:?}", virt, phys);
    }

    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator =
        unsafe { memory::BootInfoFrameAllocator::init(&boot_info.memory_map) };

    let page: Page = Page::containing_address(VirtAddr::new(0));
    memory::create_example_mapping(page, &mut mapper, &mut frame_allocator);

    let page_ptr: *mut u64 = page.start_address().as_mut_ptr();
    unsafe { page_ptr.offset(400).write_volatile(0x_f021_f077_f065_f04e) };

    // 条件编译：仅在测试配置下执行
    // 调用测试主函数，运行内核中的测试用例
    #[cfg(test)]
    test_main();

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("init_heap failed");

    // 在堆上分配数字
    let heap_value = Box::new(41);
    println!("heap_value at {:p}", heap_value);
    let heap_value = Box::new(41);
    println!("heap_value at {:p}", heap_value);
    // 创建动态大小向量
    let mut vec = Vec::with_capacity(500);
    for i in 0..500 {
        vec.push(i);
    }
    println!("vec at {:p}", vec.as_slice());

    // 创建引用计数向量，计数为 0 时释放
    let reference_counted = Rc::new(vec![1, 2, 3]);
    let cloned_reference = reference_counted.clone();
    println!(
        "current reference count is {}",
        Rc::strong_count(&cloned_reference)
    );
    core::mem::drop(reference_counted);
    println!(
        "reference count is {} now",
        Rc::strong_count(&cloned_reference)
    );
    // 打印消息，表示内核成功执行到此处而没有崩溃
    println!("It did not crash!");

    // 进入 HLT 循环，使 CPU 进入低功耗状态，等待中断唤醒
    // 这是一个无限循环，内核会一直停留在这个状态直到收到中断
    os_rust::hlt_loop();
}

/// 这个函数将在 panic 时被调用
#[cfg(not(test))] // new attribute
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    os_rust::hlt_loop();
}

// our panic handler in test mode
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    os_rust::test_panic_handler(info)
}
