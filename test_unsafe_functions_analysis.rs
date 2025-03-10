// 自动生成的Rust代码文件 - 包含不安全函数调用路径分析结果
// 此文件可以被编译器解析，具有语法高亮

// 注意：此文件仅用于查看，不应直接编译或运行
// 生成时间: 2025-03-10 15:55:39

#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(non_snake_case)]

// 分析结果开始

// ============================================================
// 文件: C:\Users\ROG\Desktop\analysis\test_unsafe_functions.rs
// ============================================================

pub mod test_unsafe_functions {
    // 发现 1 组通向不安全函数的路径

    // 组 1: 通向不安全函数的路径: test_ptr_functions
    pub mod group_1 {
        // 路径列表:
        // 1.1 pub public_caller -> test_ptr_functions

        // 其他函数实现:
        // 公共入口点: public_caller
        pub fn public_caller() {
            test_ptr_functions();
        }

        // 不安全实现: test_ptr_functions
        fn test_ptr_functions() {
            let src = [1, 2, 3, 4];
            let mut dst = [0, 0, 0, 0];
            unsafe {
                core::ptr::copy(src.as_ptr(), dst.as_mut_ptr(), src.len());
            }
            unsafe {
                std::ptr::copy(src.as_ptr(), dst.as_mut_ptr(), src.len());
            }
            unsafe {
                std::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), src.len());
            }
        }

    } // end of module group_1
} // end of module test_unsafe_functions

