// 自动生成的Rust代码文件 - 包含不安全函数调用路径分析结果
// 此文件可以被编译器解析，具有语法高亮

// 注意：此文件仅用于查看，不应直接编译或运行
// 生成时间: 2025-03-10 21:14:30

#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(non_snake_case)]

// 分析结果开始

// ============================================================
// 文件: test/test_unsafe.rs
// ============================================================

pub mod test_unsafe {
    // 发现 5 组通向不安全函数的路径

    // 组 1: 通向不安全函数的路径: public_with_ptr_copy
    pub mod group_1 {
        // 路径列表:
        // 1.1 pub public_with_ptr_copy

        // 其他函数实现:
        // 公共入口点: public_with_ptr_copy
        // 不安全操作：
        //            1. 代码: std :: ptr :: copy (src . as_ptr () , dst . as_mut_ptr () , src . len ())
        //            2. 代码: src . as_ptr ()
        //            3. 代码: dst . as_mut_ptr ()
        pub fn public_with_ptr_copy() {
            let src = [1, 2, 3, 4];
            let mut dst = [0, 0, 0, 0];
            unsafe {
                std::ptr::copy(src.as_ptr(), dst.as_mut_ptr(), src.len());
            }
        }

    } // end of module group_1

    // 组 2: 通向不安全函数的路径: public_with_raw_ptr_deref
    pub mod group_2 {
        // 路径列表:
        // 2.1 pub public_with_raw_ptr_deref

        // 其他函数实现:
        // 公共入口点: public_with_raw_ptr_deref
        // 不安全操作：
        //            1. 代码: * ptr
        pub fn public_with_raw_ptr_deref() -> i32 {
            let x = 42;
            let ptr = &x as *const i32;
            unsafe { *ptr }
        }

    } // end of module group_2

    // 组 3: 通向不安全函数的路径: public_method_with_ptr_deref
    pub mod group_3 {
        // 路径列表:
        // 3.1 pub public_method_with_ptr_deref


        // 相关自定义类型定义:
        // 类型: TestStruct
        pub struct TestStruct {
        value: i32,
        }

        impl TestStruct {

            // 公共入口点: public_method_with_ptr_deref
            // 不安全操作：
            //            1. 代码: * ptr
            pub fn public_method_with_ptr_deref(&self) -> i32 {
                let ptr = &self.value as *const i32;
                unsafe { *ptr }
            }
        }

    } // end of module group_3

    // 组 4: 通向不安全函数的路径: public_with_unsafe_inside
    pub mod group_4 {
        // 路径列表:
        // 4.1 pub public_with_unsafe_inside

        // 其他函数实现:
        // 公共入口点: public_with_unsafe_inside
        pub fn public_with_unsafe_inside() -> *const i32 {
            let x = 42;
            unsafe { &x as *const i32 }
        }

    } // end of module group_4

    // 组 5: 通向不安全函数的路径: public_method_with_unsafe
    pub mod group_5 {
        // 路径列表:
        // 5.1 pub public_method_with_unsafe


        // 相关自定义类型定义:
        // 类型: TestStruct
        pub struct TestStruct {
        value: i32,
        }

        impl TestStruct {

            // 公共入口点: public_method_with_unsafe
            pub fn public_method_with_unsafe(&self) -> *const i32 {
                unsafe { &self.value as *const i32 }
            }
        }

    } // end of module group_5
} // end of module test_unsafe

