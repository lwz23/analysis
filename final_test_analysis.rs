// 自动生成的Rust代码文件 - 包含不安全函数调用路径分析结果
// 此文件可以被编译器解析，具有语法高亮

// 注意：此文件仅用于查看，不应直接编译或运行
// 生成时间: 2025-03-10 15:57:35

#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(non_snake_case)]

// 分析结果开始

// ============================================================
// 文件: C:\Users\ROG\Desktop\analysis\final_test.rs
// ============================================================

pub mod final_test {
    // 发现 2 组通向不安全函数的路径

    // 组 1: 通向不安全函数的路径: test_copy
    pub mod group_1 {
        // 路径列表:
        // 1.1 pub public_function -> test_copy

        // 其他函数实现:
        // 公共入口点: public_function
        pub fn public_function() {
            test_copy();
            test_copy_nonoverlapping();
        }

        // 不安全实现: test_copy
        fn test_copy() {
            let src = [1, 2, 3, 4];
            let mut dst = [0, 0, 0, 0];
            unsafe {
                std::ptr::copy(src.as_ptr(), dst.as_mut_ptr(), 4);
            }
        }

    } // end of module group_1

    // 组 2: 通向不安全函数的路径: test_copy_nonoverlapping
    pub mod group_2 {
        // 路径列表:
        // 2.1 pub public_function -> test_copy_nonoverlapping

        // 其他函数实现:
        // 公共入口点: public_function
        pub fn public_function() {
            test_copy();
            test_copy_nonoverlapping();
        }

        // 不安全实现: test_copy_nonoverlapping
        fn test_copy_nonoverlapping() {
            let src = [1, 2, 3, 4];
            let mut dst = [0, 0, 0, 0];
            unsafe {
                std::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), 4);
            }
        }

    } // end of module group_2
} // end of module final_test

