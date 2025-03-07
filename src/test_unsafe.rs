pub struct Queue<T> {
    data: Vec<T>,
}

impl<T> Queue<T> {
    pub fn new() -> Self {
        Queue {
            data: Vec::new(),
        }
    }

    pub fn send_back(&self, item: T, timeout: u32) -> Result<bool, &'static str> {
        self.send_generic(item, timeout, 0)
    }

    fn send_generic(&self, item: T, timeout: u32, flags: u8) -> Result<bool, &'static str> {
        // 调用不安全函数
        unsafe { self.send_unsafe(item, timeout, flags) }
    }

    unsafe fn send_unsafe(&self, item: T, timeout: u32, flags: u8) -> Result<bool, &'static str> {
        // 这里是不安全的实现
        Ok(true)
    }
} 