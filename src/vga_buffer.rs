// 编译器忽略未使用代码的警告
use volatile::Volatile;
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// Rust 枚举的底层类型是平台相关的（通常是 isize 或 usize）。通过使用
// #[repr(u8)]，你指定枚举变体的底层表示应该使用 u8，即每个枚举变体将存储为 1 字节。 #[repr(u8)]
// 只是指定了枚举的具体底层类型（u8），而 #[repr(C)]
// 则更加关注结构体或枚举在内存中的排列方式（例如确保按字节对齐、填充等）。
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
// 如果一个结构体只包含一个标量字段（如 u8），它的内存布局非常简单。Rust
// 会确保该结构体与该字段的布局一致，而不会进行其他优化或添加额外的填充。此时，#
// [repr(C)] 并不是必需的，因为结构体的内存布局没有复杂性，且它与 C 的 uint8_t
// 类型是兼容的。
// 结构体 ColorCode 中只有一个 u8 字段，所以它的大小和内存布局与 u8 完全相同。在
// ColorCode 中的 u8 字段可以通过直接访问结构体的字段进行转换，或者通过实现 From
// 或 Into trait 来支持隐式转换。
struct ColorCode(u8);
impl ColorCode {
    fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

#[repr(transparent)]
// 如果没有 #[repr(transparent)]，你就不能直接将 Buffer 作为 ScreenChar
// 数组来处理。例如，&Buffer 和 &Buffer.chars 的类型可能会不同，但通过
// #[repr(transparent)]，&Buffer 和 &Buffer.chars
// 的类型在内存中是一致的，这样你可以方便地操作和传递数组。
struct Buffer {
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}
pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
    // 我们对借用使用显式生命周期（explicit lifetime），告诉编译器这个借用在何时有效
    buffer: &'static mut Buffer,
}

impl Writer {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;

                let color_code = self.color_code;
                self.buffer.chars[row][col].write(ScreenChar {
                    ascii_character: byte,
                    color_code,
                });
                self.column_position += 1;
            }
        }
    }

    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // 可打印的 ASCII 字符（0x20 空格到 0x7e ~）
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                // 不可打印的字符用 ■ 替代
                _ => self.write_byte(0xfe),
            }
        }
    }
    pub fn new_line(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let character = self.buffer.chars[row][col].read();
                self.buffer.chars[row - 1][col].write(character);
            }
        }
        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
    }

    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };
        for col in 0..BUFFER_WIDTH {
            self.buffer.chars[row][col].write(blank);
        }
    }
}
#[allow(dead_code)]
pub fn print_something() {
    use core::fmt::Write;
    let mut writer = Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    };

    writer.write_byte(b'H');
    writer.write_string("ello ");
    writer.write_string("Wörld!");
    write!(writer, "The numbers are {} and {}", 42, 1.0 / 3.0).unwrap();
}

use core::fmt;
impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

use lazy_static::lazy_static;
use spin::Mutex;
lazy_static! {
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    });
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

// 如果这个宏将能在模块外访问，它们也应当能访问 _print
// 函数，因此这个函数必须是公有的（public）。然而，
// 考虑到这是一个私有的实现细节，我们添加一个 doc(hidden)
// 属性，防止它在生成的文档中出现。
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;

    use x86_64::instructions::interrupts;
    interrupts::without_interrupts(|| {
        WRITER.lock().write_fmt(args).unwrap();
    });
}

#[test_case]
fn test_println_simple() {
    println!("test_println_simple output");
}

#[test_case]
fn test_println_many() {
    for _ in 0..200 {
        println!("test_println_many output");
    }
}

#[test_case]
fn test_println_output() {
    use core::fmt::Write;

    use x86_64::instructions::interrupts;
    let s = "Some test string that fits on a single line";
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        #[allow(clippy::uninlined_format_args)]
        writeln!(writer, "\n{}", s).expect("writeln failed");
        for (i, c) in s.chars().enumerate() {
            let screen_char = WRITER.lock().buffer.chars[BUFFER_HEIGHT - 2][i].read();
            // 从 u8 转换为 char
            assert_eq!(char::from(screen_char.ascii_character), c);
        }
    });
}
