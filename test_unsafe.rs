// 测试文件：包含多种不同的Rust函数类型

// 例子1：公共函数，内部有unsafe操作（应该被我们的工具识别）
pub fn public_with_unsafe_inside() -> *const i32 {
    let x = 42;
    unsafe {
        &x as *const i32
    }
}

// 例子2：公共unsafe函数（不应该被识别，因为它明确声明为unsafe）
pub unsafe fn public_unsafe_fn() -> *const i32 {
    let x = 10;
    &x as *const i32
}

// 例子3：私有函数，内部有unsafe操作（不应该被识别，因为它不是公共的）
fn private_with_unsafe_inside() -> *const i32 {
    let x = 5;
    unsafe {
        &x as *const i32
    }
}

// 例子4：公共函数，调用私有不安全函数（原代码会识别，但修改后不会）
pub fn public_calling_private_unsafe() -> *const i32 {
    private_with_unsafe_inside()
}

// 例子5：完全安全的公共函数（不应该被识别）
pub fn safe_public_function() -> i32 {
    42
}

// 结构体方法测试
pub struct TestStruct {
    value: i32
}

impl TestStruct {
    // 例子6：公共方法，内部有unsafe操作（应该被识别）
    pub fn public_method_with_unsafe(&self) -> *const i32 {
        unsafe {
            &self.value as *const i32
        }
    }
    
    // 例子7：私有方法，内部有unsafe操作（不应该被识别）
    fn private_method_with_unsafe(&self) -> *const i32 {
        unsafe {
            &self.value as *const i32
        }
    }
}

pub fn public_calling_private_unsafe(byte: u8) {
    let x = unsafe {from_utf8_unchecked(&[byte])};
    println!("{}", x);
}

fn main() {
    println!("测试文件，用于验证不安全路径分析工具");
} 