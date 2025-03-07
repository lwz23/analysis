// 自动生成的Rust代码文件 - 包含不安全函数调用路径分析结果
// 此文件可以被编译器解析，具有语法高亮

// 注意：此文件仅用于查看，不应直接编译或运行
// 生成时间: 2025-03-07 09:14:33

#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(non_snake_case)]

// 分析结果开始

// ============================================================
// 文件: E:\Github\unsafe_rust_project_all\unsafe_rust_download\download_rs\below_1k\esp-idf-hal\280ae0099a4bc3c8503f26273046dd13746b7a35\src\task.rs
// ============================================================

pub mod task {
    // 发现 2 组通向不安全函数的路径

    // 组 1: 通向不安全函数的路径: get_conf
    pub mod group_1 {
        // 路径列表:
        // 1.1 pub fn get -> fn get_conf
        // 其他函数实现:
        // 公共入口点: thread::get
        pub fn get() -> Option<Self> {
            get_conf()
        }

        // 不安全实现: thread::get_conf
        fn get_conf() -> Option<ThreadSpawnConfiguration> {
            let mut conf: esp_pthread_cfg_t = Default::default();
            let res = unsafe { esp_pthread_get_cfg(&mut conf as _) };
            if res == ESP_ERR_NOT_FOUND { None } else { Some(conf.into()) }
        }

    } // end of module group_1

    // 组 2: 通向不安全函数的路径: send_generic
    pub mod group_2 {
        // 路径列表:
        // 2.1 pub fn send_front -> fn send_generic
        // 2.2 pub fn send_back -> fn send_generic

        // 相关自定义类型定义:
        // 类型: queue::Queue
        pub struct Queue<T> {
        ptr: sys::QueueHandle_t,
        is_owned: bool,
        _marker: PhantomData<T>,
        }

        impl Queue {
            pub fn new(size: usize) -> Self {
            Queue {
            ptr: unsafe {
            sys::xQueueGenericCreate(size as u32, size_of::<T>() as u32, 0)
            },
            is_owned: true,
            _marker: PhantomData,
            }
            }
        }

        // 其他函数实现:
        // 公共入口点: queue::send_front
        #[inline]
        #[link_section = "iram1.queue_send_front"]
        pub fn send_front(&self, item: T, timeout: TickType_t) -> Result<bool, EspError> {
            self.send_generic(item, timeout, 1)
        }

        // 公共入口点: queue::send_back
        #[inline]
        #[link_section = "iram1.queue_send_back"]
        pub fn send_back(&self, item: T, timeout: TickType_t) -> Result<bool, EspError> {
            self.send_generic(item, timeout, 0)
        }

        // 不安全实现: queue::send_generic
        #[inline]
        #[link_section = "iram1.queue_send_generic"]
        fn send_generic(
            &self,
            item: T,
            timeout: TickType_t,
            copy_position: i32,
        ) -> Result<bool, EspError> {
            let mut hp_task_awoken: i32 = false as i32;
            let success = unsafe {
                if crate::interrupt::active() {
                    sys::xQueueGenericSendFromISR(
                        self.ptr,
                        &item as *const T as *const _,
                        &mut hp_task_awoken,
                        copy_position,
                    )
                } else {
                    sys::xQueueGenericSend(
                        self.ptr,
                        &item as *const T as *const _,
                        timeout,
                        copy_position,
                    )
                }
            };
            let success = success == 1;
            let hp_task_awoken = hp_task_awoken == 1;
            match success {
                true => Ok(hp_task_awoken),
                false => Err(EspError::from_infallible::<ESP_FAIL>()),
            }
        }

    } // end of module group_2
} // end of module task

