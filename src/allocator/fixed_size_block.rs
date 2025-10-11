use alloc::alloc::{GlobalAlloc, Layout};
use core::mem;

use super::Locked;
struct ListNode {
    next: Option<&'static mut ListNode>,
}

const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];

pub struct FixedSizeBlockAllocator {
    list_heads: [Option<&'static mut ListNode>; BLOCK_SIZES.len()],
    fallback_allocator: linked_list_allocator::Heap,
}

impl Default for FixedSizeBlockAllocator {
    fn default() -> Self {
        Self::new()
    }
}
impl FixedSizeBlockAllocator {
    pub const fn new() -> Self {
        const EMPTY: Option<&'static mut ListNode> = None;
        FixedSizeBlockAllocator {
            list_heads: [EMPTY; BLOCK_SIZES.len()],
            fallback_allocator: linked_list_allocator::Heap::empty(),
        }
    }
    /// # Safety
    /// 这个方法是不安全的，因为调用者必须确保给定
    /// 的内存范围没有被使用。同样，这个方法只能被调用一次。
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        unsafe { self.fallback_allocator.init(heap_start, heap_size) }
    }
    fn fallback_alloc(&mut self, layout: Layout) -> *mut u8 {
        match self.fallback_allocator.allocate_first_fit(layout) {
            Ok(ptr) => ptr.as_ptr(),
            Err(_) => core::ptr::null_mut(),
        }
    }
}
/// 根据布局信息，返回合适的块列表索引
fn list_index(layout: &Layout) -> Option<usize> {
    let required_block_size = layout.size().max(layout.align());
    BLOCK_SIZES.iter().position(|&s| s >= required_block_size)
}

unsafe impl GlobalAlloc for Locked<FixedSizeBlockAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut allocator = self.lock();
        match list_index(&layout) {
            Some(index) => {
                match allocator.list_heads[index].take() {
                    Some(node) => {
                        allocator.list_heads[index] = node.next.take();
                        node as *mut ListNode as *mut u8
                    }
                    None => {
                        // 没有空闲块，分配一个新的块
                        let block_size = BLOCK_SIZES[index];
                        let layout = Layout::from_size_align(block_size, block_size).unwrap();
                        allocator.fallback_alloc(layout)
                    }
                }
            }
            None => allocator.fallback_alloc(layout),
        }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut allocator = self.lock();
        match list_index(&layout) {
            Some(index) => {
                let node = ListNode {
                    next: allocator.list_heads[index].take(),
                };
                // 验证块是否满足存储节点所需的大小和对齐方式要求
                assert!(mem::size_of::<ListNode>() <= BLOCK_SIZES[index]);
                assert!(mem::align_of::<ListNode>() <= BLOCK_SIZES[index]);
                let node_ptr = ptr as *mut ListNode;
                unsafe {
                    node_ptr.write(node);
                    allocator.list_heads[index] = Some(&mut *node_ptr);
                }
            }
            None => {
                let layout = Layout::from_size_align(layout.size(), layout.align()).unwrap();
                unsafe {
                    allocator
                        .fallback_allocator
                        .deallocate(core::ptr::NonNull::new(ptr).unwrap(), layout)
                }
            }
        }
    }
}
