use alloc::alloc::{GlobalAlloc, Layout};
use core::{mem, ptr};

use super::{Locked, align_up};

struct ListNode {
    size: usize,
    // 节点可能分配在 静态缓冲区 或堆管理的内存池中
    // 因此用 'static 表示引用 在整个程序运行期间都是有效的。
    next: Option<&'static mut ListNode>,
}

impl ListNode {
    const fn new(size: usize) -> Self {
        ListNode { size, next: None }
    }
    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}
pub struct LinkedListAllocator {
    head: ListNode,
}
impl Default for LinkedListAllocator {
    fn default() -> Self {
        Self::new()
    }
}
impl LinkedListAllocator {
    pub const fn new() -> Self {
        LinkedListAllocator {
            head: ListNode::new(0),
        }
    }
    /// 用给定的堆边界初始化分配器
    /// # Safety
    /// 这个函数是不安全的，因为调用者必须保证给定的堆边界是有效的并且堆是未使用的。
    /// 此方法只能调用一次
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        unsafe {
            self.add_free_region(heap_start, heap_size);
        }
    }
    /// 将给定的内存区域添加到链表前端。
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        // 确保给定的内存区域足以存储ListNode
        // 保证地址对齐到ListNode的对齐要求
        assert_eq!(align_up(addr, mem::align_of::<ListNode>()), addr);
        // 确保给定的内存区域大小足够大
        assert!(size >= mem::size_of::<ListNode>());
        // 创建一个新的ListNode并添加到链表前端
        let mut node = ListNode::new(size);
        // 将我这个指针初始化为第一指针的地址
        node.next = self.head.next.take();
        // 将我这个指针初始化为第一指针的地址
        let node_ptr = addr as *mut ListNode;
        unsafe {
            node_ptr.write(node);
            self.head.next = Some(&mut *node_ptr);
        }
    }

    /// 查找给定大小和对齐方式的空闲区域并将其从链表中移除。
    ///
    /// 返回一个包含链表节点和分配内存区域起始地址的元组。
    fn find_region(&mut self, size: usize, align: usize) -> Option<(&'static mut ListNode, usize)> {
        // 当前链表节点的引用，每次迭代更新
        let mut current = &mut self.head;
        // 从链表中查找合适大小的内存区域
        while let Some(ref mut region) = current.next {
            // 区域适用于分配 -> 从链表中移除该节点
            if let Ok(alloc_start) = Self::alloc_from_region(region, size, align) {
                let next = region.next.take();
                let ret = Some((current.next.take().unwrap(), alloc_start));
                current.next = next;
                return ret;
            } else {
                // 区域不适合
                // 快速取Option中的引用，unwrap() 是安全的，因为我们已经检查过 next 不是 None。
                current = current.next.as_mut().unwrap();
            }
        }
        None
    }

    fn alloc_from_region(region: &ListNode, size: usize, align: usize) -> Result<usize, ()> {
        // 对齐要求
        let alloc_start = align_up(region.start_addr(), align);
        // 安全加法None → 发生溢出。
        // ok_or(())?
        // 是否满足分配要求
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;
        if alloc_end > region.end_addr() {
            return Err(());
        }
        // 剩下区域是否可以存储结构体
        // 分配器需要 继续管理未使用的内存，否则这部分会“丢失”。
        // 要么分配完全适配（excess_size == 0），要么剩余空间足以存储一个 ListNode 。
        let excess_size = region.end_addr() - alloc_end;
        if excess_size > 0 && excess_size < mem::size_of::<ListNode>() {
            return Err(());
        }
        Ok(alloc_start)
    }
    fn size_align(layout: Layout) -> (usize, usize) {
        let layout = layout
        // 功能：调整 layout 的对齐方式，使其符合 ListNode 的对齐要求。
            .align_to(mem::align_of::<ListNode>())
            .expect("adjusting failed")
            // 调整 layout.size，确保总大小是对齐的整数倍。
            .pad_to_align();
        let size = layout.size().max(mem::size_of::<ListNode>());
        (size, layout.align())
    }
}

unsafe impl GlobalAlloc for Locked<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // 执行布局调整
        let (size, align) = LinkedListAllocator::size_align(layout);
        let mut alocator = self.lock();
        if let Some((region, alloc_start)) = alocator.find_region(size, align) {
            // TODO 分配的时候就check过layout了，这里就不需要check了
            let alloc_end = alloc_start.checked_add(size).expect("overflow");
            let excess_size = region.end_addr() - alloc_end;
            if excess_size > 0 {
                unsafe {
                    alocator.add_free_region(alloc_end, excess_size);
                }
            }

            return alloc_start as *mut u8;
        }
        ptr::null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // 执行布局调整
        let (size, _align) = LinkedListAllocator::size_align(layout);
        unsafe { self.lock().add_free_region(ptr as usize, size) }
    }
}
