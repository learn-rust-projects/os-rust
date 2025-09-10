use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PhysFrame, Size4KiB,
    },
};

/// # Safety
/// 此函数是 unsafe 的，因为它涉及直接的内存操作和指针转换，
/// 这些操作可能导致未定义行为，需要调用者确保物理内存偏移量正确且有效。
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    unsafe {
        let level_4_table = active_level_4_table(physical_memory_offset);
        OffsetPageTable::new(level_4_table, physical_memory_offset)
    }
}

/// 获取当前活动的 Level 4 页表（PML4）的可变引用
///
/// 在 x86_64 架构中，Level 4 页表是页表层次结构的最高层，
/// 用于实现虚拟内存到物理内存的映射。
///
/// # 参数
/// - `physical_memory_offset`: 物理内存偏移量，用于将物理地址转换为虚拟地址
///
/// # 返回值
/// 指向当前活动的 Level 4 页表的可变引用，具有 'static 生命周期
///
/// # Safety
/// 此函数是 unsafe 的，因为它涉及直接的内存操作和指针转换，
/// 这些操作可能导致未定义行为，需要调用者确保物理内存偏移量正确且有效。
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    // 导入 Cr3 寄存器相关功能，用于读取当前页表的物理地址
    use x86_64::registers::control::Cr3;

    // 读取 CR3 控制寄存器，获取当前活动页表的物理地址信息
    // CR3 寄存器存储了页表的物理地址和其他页表相关控制位
    let (level_4_table_frame, _) = Cr3::read();

    // 提取页表的起始物理地址
    let phys = level_4_table_frame.start_address();

    // 将物理地址转换为虚拟地址，通过加上物理内存偏移量
    // 这是因为在 64 位操作系统中，我们通常使用虚拟地址空间访问内存
    let virt = physical_memory_offset + phys.as_u64();

    // 将虚拟地址转换为指向 PageTable 类型的可变原始指针
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    // 解引用原始指针，获取对页表的可变引用
    // 这里再次使用 unsafe 块，因为解引用原始指针是不安全操作
    unsafe { &mut *page_table_ptr }
}

pub fn create_example_mapping(
    page: Page,
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    use x86_64::structures::paging::PageTableFlags as Flags;
    let frame = PhysFrame::containing_address(PhysAddr::new(0xb8000));
    let flags = Flags::PRESENT | Flags::WRITABLE;

    let map_to_result = unsafe {
        // FIXME: 这并不安全，我们这样做只是为了测试。
        mapper.map_to(page, frame, flags, frame_allocator)
    };
    // 调用 flush() 可以使 CPU 更新 TLB，保证新的虚拟-物理映射生效。
    map_to_result.expect("map_to failed").flush();
}

pub struct EmptyFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for EmptyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        None
    }
}

use bootloader::bootinfo::MemoryMap;
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

impl BootInfoFrameAllocator {
    /// # Safety
    /// 此函数是 unsafe 的，因为它涉及直接的内存操作和指针转换，
    /// 这些操作可能导致未定义行为，需要调用者确保物理内存偏移量正确且有效。
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        Self {
            memory_map,
            next: 0,
        }
    }

    fn unsable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        use bootloader::bootinfo::MemoryRegionType;
        // 从内存map中获取可用的区域
        let regions = self.memory_map.iter();
        // 过滤出来可以使用的
        let usable_regions = regions.filter(|r| r.region_type == MemoryRegionType::Usable);
        // 将每个区域映射到其地址范围
        let addr_ranges = usable_regions.map(|r| r.range.start_addr()..r.range.end_addr());
        // 转化为一个帧起始地址的迭代器
        // Bootloader对所有可用的内存区域进行页对齐，
        // 所以我们在这里不需要任何对齐或舍入代码 Bootloader
        // 在传递可用内存信息时，会保证所有内存区域已经页对齐，即：
        // 起始地址是 4 KiB 的整数倍。
        // 结束地址也是 4 KiB 的整数倍。
        // 因此在内核中可以直接用 step_by(4096) 遍历帧，无需手动对齐或舍入。
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        // 从起始地址创建 `PhysFrame`  类型
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let frame = self.unsable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}
