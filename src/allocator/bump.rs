pub struct BumpAllocator {
    head_start: usize,
    head_end: usize,
    next: usize,
    allocations: usize,
}

impl Default for BumpAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl BumpAllocator {
    // 让 new() 可以在编译期用来初始化 const 或 static，提高性能和灵活性。
    pub const fn new() -> Self {
        Self {
            head_start: 0,
            head_end: 0,
            next: 0,
            allocations: 0,
        }
    }

    /// 用给定的堆边界初始化bump分配器
    /// # Safety
    /// 这个方法是不安全的，因为调用者必须确保给定
    /// 的内存范围没有被使用。同样，这个方法只能被调用一次。
    pub unsafe fn init(&mut self, head_start: usize, heap_size: usize) {
        self.head_start = head_start;
        self.head_end = head_start + heap_size;
        self.next = head_start;
    }
}
use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr;

use super::{Locked, align_up};

unsafe impl GlobalAlloc for Locked<BumpAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // 第一步都是调用 Mutex::lock 方法来通过 inner 字段获取封装类型的可变引用
        // 封装实例在方法结束前保持锁定，
        // 因此不会在多线程上下文中发生数据竞争（我们很快会添加线程支持）。
        let mut bump = self.lock();
        // 将 next 地址向上对齐到 Layout 参数指定的对齐值
        // 确保分配的内存地址满足对齐要求
        let alloc_start = align_up(bump.next, layout.size());
        let alloc_end = match alloc_start.checked_add(layout.size()) {
            Some(end) => end,
            None => return ptr::null_mut(),
        };

        if alloc_end > bump.head_end {
            ptr::null_mut()
        } else {
            bump.next = alloc_end;
            bump.allocations += 1;
            alloc_start as *mut u8
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut bump = self.lock(); // 获取可变引用
        // 释放最后一块分配的内存可以跑通测试
        // 更新 dealloc 方法，通过比较其结束地址与 next 指针来检查释放的分配是否与 alloc
        // 返回的最后一个分配的结束地址相等
        if ptr::eq(ptr, (bump.next - layout.size()) as *const u8) {
            bump.next = ptr as usize;
        }
        bump.allocations -= 1;
        if bump.allocations == 0 {
            bump.next = bump.head_start;
        }
    }
}
