use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spin;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

use crate::{gdt, print, println};

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);

       unsafe {
               idt.double_fault.set_handler_fn(double_fault_handler)
               .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX );
                }

         // 注册时 cast 成期望类型
         // IndexMut 特征，因此我们可以通过数组索引语法访问单个条目
         // 因为中断向量号本身就是 u8（0–255）
       idt[InterruptIndex::Timer.as_u8()].set_handler_fn(timer_interrupt_handler);
       idt[InterruptIndex::Keyboard.as_u8()].set_handler_fn(keyboard_interrupt_handler);

    idt
    };
}

pub fn init_idt() {
    IDT.load();
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}
// CPU 对异常和外部中断的反应相同（唯一的区别是某些异常会推送错误代码）
extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    print!(".");
    unsafe {
        // 我们需要小心使用正确的中断向量号，
        // 否则我们可能会意外删除重要的未发送中断或导致我们的系统挂起。
        // 这就是该功能不安全的原因。
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // 键盘按下产生 扫描码 (scan code)，键盘控制器把它放到 输出缓冲区 (Output
    // Buffer)。
    // 同时，键盘控制器会向 CPU 发送 中断请求 (IRQ1)。
    // CPU 响应中断后，内核的键盘中断处理函数会读取扫描码。
    // 关键点：在你读取扫描码之前，键盘控制器不会发送新的中断。
    // 换句话说，如果缓冲区里还有未读取的数据，中断不会再触发。
    use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1, layouts};
    use spin::Mutex;
    use x86_64::instructions::port::Port;

    lazy_static! {
        static ref KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> =
            Mutex::new(Keyboard::new(
                ScancodeSet1::new(),
                layouts::Us104Key,
                HandleControl::Ignore
            ));
    }
    let mut keyboard = KEYBOARD.lock();
    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    // Option<KeyEvent> 结构。KeyEvent
    // 包括了触发本次中断的按键信息，以及子动作是按下还是释放。
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        // 要处理KeyEvent，我们还需要将其传入 process_keyevent
        // 函数，将其转换为人类可读的字符
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(character) => print!("{}", character),
                DecodedKey::RawKey(key) => print!("{:?}", key),
            }
        }
    }
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

#[test_case]
fn test_breakpoint_exception() {
    // invoke a breakpoint exception
    x86_64::instructions::interrupts::int3();
}

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    // Keyboard 没有显式赋值，所以 Rust 会自动给它赋值 紧接上一个值 +1。
    Keyboard,
}

impl InterruptIndex {
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}
