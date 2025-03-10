// 测试文件：包含多种不同的Rust函数类型和不安全操作

// 例子1：公共函数，内部有unsafe操作（应该被我们的工具识别）
pub fn public_with_unsafe_inside() -> *const i32 {
    let x = 42;
    unsafe {
        &x as *const i32
    }
}

// 例子2：公共函数，内部解引用裸指针（应该被我们的工具识别）
pub fn public_with_raw_ptr_deref() -> i32 {
    let x = 42;
    let ptr = &x as *const i32;
    unsafe {
        *ptr // 解引用裸指针
    }
}

// 例子3：公共函数，使用ptr::copy（应该被我们的工具识别）
pub fn public_with_ptr_copy() {
    let src = [1, 2, 3, 4];
    let mut dst = [0, 0, 0, 0];
    
    unsafe {
        std::ptr::copy(src.as_ptr(), dst.as_mut_ptr(), src.len());
    }
}

// 例子4：公共unsafe函数（不应该被识别，因为它明确声明为unsafe）
pub unsafe fn public_unsafe_fn() -> *const i32 {
    let x = 10;
    &x as *const i32
}

// 例子5：私有函数，内部有unsafe操作（不应该被识别，因为它不是公共的）
fn private_with_unsafe_inside() -> *const i32 {
    let x = 5;
    unsafe {
        &x as *const i32
    }
}

// 例子6：公共函数，调用私有不安全函数（原代码会识别，但修改后不会）
pub fn public_calling_private_unsafe() -> *const i32 {
    private_with_unsafe_inside()
}

// 例子7：完全安全的公共函数（不应该被识别）
pub fn safe_public_function() -> i32 {
    42
}

// 结构体方法测试
pub struct TestStruct {
    value: i32
}

impl TestStruct {
    // 例子8：公共方法，内部有unsafe操作（应该被识别）
    pub fn public_method_with_unsafe(&self) -> *const i32 {
        unsafe {
            &self.value as *const i32
        }
    }
    
    // 例子9：公共方法，解引用裸指针（应该被识别）
    pub fn public_method_with_ptr_deref(&self) -> i32 {
        let ptr = &self.value as *const i32;
        unsafe {
            *ptr // 解引用裸指针
        }
    }
    
    // 例子10：私有方法，内部有unsafe操作（不应该被识别）
    fn private_method_with_unsafe(&self) -> *const i32 {
        unsafe {
            &self.value as *const i32
        }
    }
}

fn main() {
    println!("测试文件，用于验证不安全路径分析工具");
} 