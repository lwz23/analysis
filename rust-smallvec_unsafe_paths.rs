// 自动生成的Rust代码文件 - 包含不安全函数调用路径分析结果
// 此文件可以被编译器解析，具有语法高亮

// 注意：此文件仅用于查看，不应直接编译或运行
// 生成时间: 2025-03-10 16:31:47

#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(non_snake_case)]

// 分析结果开始

// ============================================================
// 文件: E:\Github\rust-smallvec\src\lib.rs
// ============================================================

pub mod lib {
    // 发现 3 组通向不安全函数的路径

    // 组 1: 通向不安全函数的路径: insert_many_impl
    pub mod group_1 {
        // 路径列表:
        // 1.1 pub insert_many -> insert_many_impl


        // 相关自定义类型定义:
        // 类型: SmallVec
        #[repr(C)]
        pub struct SmallVec<T, const N: usize> {
        len: TaggedLen,
        raw: RawSmallVec<T, N>,
        _marker: PhantomData<T>,
        }

        impl SmallVec {
            pub const fn new() -> SmallVec<T, N> {
            Self {
            len: TaggedLen::new(0, false, Self::is_zst()),
            raw: RawSmallVec::new(),
            _marker: PhantomData,
            }
            }
            pub fn with_capacity(capacity: usize) -> Self {
            let mut this = Self::new();
            if capacity > Self::inline_size() {
            this.grow(capacity);
            }
            this
            }
            pub fn from_vec(vec: Vec<T>) -> Self {
            if vec.capacity() == 0 {
            return Self::new();
            }
            if Self::is_zst() {
            let mut vec = vec;
            let len = vec.len();
            unsafe { vec.set_len(0) };
            Self {
            len: TaggedLen::new(len, false, Self::is_zst()),
            raw: RawSmallVec::new(),
            _marker: PhantomData,
            }
            } else {
            let mut vec = ManuallyDrop::new(vec);
            let len = vec.len();
            let cap = vec.capacity();
            let ptr = unsafe { NonNull::new_unchecked(vec.as_mut_ptr()) };
            Self {
            len: TaggedLen::new(len, true, Self::is_zst()),
            raw: RawSmallVec::new_heap(ptr, cap),
            _marker: PhantomData,
            }
            }
            }
            pub const fn from_buf(buf: [T; N]) -> Self {
            Self {
            len: TaggedLen::new(N, false, Self::is_zst()),
            raw: RawSmallVec::new_inline(MaybeUninit::new(buf)),
            _marker: PhantomData,
            }
            }
            pub fn from_buf_and_len(buf: [T; N], len: usize) -> Self {
            assert!(len <= N);
            let mut vec = Self {
            len: TaggedLen::new(len, false, Self::is_zst()),
            raw: RawSmallVec::new_inline(MaybeUninit::new(buf)),
            _marker: PhantomData,
            };
            unsafe {
            let remainder_ptr = vec.raw.as_mut_ptr_inline().add(len);
            let remainder_len = N - len;
            core::ptr::drop_in_place(
            core::ptr::slice_from_raw_parts_mut(remainder_ptr, remainder_len),
            );
            }
            vec
            }
            pub fn split_off(&mut self, at: usize) -> Self {
            let len = self.len();
            assert!(at <= len);
            let other_len = len - at;
            let mut other = Self::with_capacity(other_len);
            unsafe {
            self.set_len(at);
            other.set_len(other_len);
            core::ptr::copy_nonoverlapping(
            self.as_ptr().add(at),
            other.as_mut_ptr(),
            other_len,
            );
            }
            other
            }
            fn default() -> Self {
            Self::new()
            }
            pub fn from_slice(slice: &[T]) -> Self {
            let len = slice.len();
            if len <= Self::inline_size() {
            let mut this = Self::new();
            unsafe {
            let ptr = this.raw.as_mut_ptr_inline();
            copy_nonoverlapping(slice.as_ptr(), ptr, len);
            this.set_len(len);
            }
            this
            } else {
            let mut this = Vec::with_capacity(len);
            unsafe {
            let ptr = this.as_mut_ptr();
            copy_nonoverlapping(slice.as_ptr(), ptr, len);
            this.set_len(len);
            }
            Self::from_vec(this)
            }
            }
            pub fn from_elem(elem: T, n: usize) -> Self {
            if n > Self::inline_size() {
            Self::from_vec(vec![elem; n])
            } else {
            let mut v = Self::new();
            unsafe {
            let ptr = v.raw.as_mut_ptr_inline();
            let mut guard = DropGuard { ptr, len: 0 };
            for i in 0..n {
            guard.len = i;
            ptr.add(i).write(elem.clone());
            }
            core::mem::forget(guard);
            v.set_len(n);
            }
            v
            }
            }
            fn from_iter<I: IntoIterator<Item = T>>(iterable: I) -> Self {
            let mut vec = Self::new();
            vec.extend_impl(iterable.into_iter());
            vec
            }
            default fn spec_from(slice: &[Self::Element]) -> Self {
            slice.iter().cloned().collect()
            }
            fn spec_from(slice: &[Self::Element]) -> Self {
            Self::from_slice(slice)
            }
            fn from(slice: &'a [T]) -> Self {
            <Self as SpecFrom>::spec_from(slice)
            }
            fn from(slice: &'a [T]) -> Self {
            slice.iter().cloned().collect()
            }
            fn from(array: [T; M]) -> Self {
            if M > N {
            Self::from(Vec::from(array))
            } else {
            let mut this = Self::new();
            debug_assert!(M <= this.capacity());
            let array = ManuallyDrop::new(array);
            unsafe {
            copy_nonoverlapping(array.as_ptr(), this.as_mut_ptr(), M);
            this.set_len(M);
            }
            this
            }
            }
            fn from(array: Vec<T>) -> Self {
            Self::from_vec(array)
            }
            fn clone(&self) -> SmallVec<T, N> {
            SmallVec::from(self.as_slice())
            }

            // 公共入口点: insert_many
            pub fn insert_many<I: IntoIterator<Item = T>>(&mut self, index: usize, iterable: I) {
            self.insert_many_impl(index, iterable.into_iter());
            }

            // 不安全实现: insert_many_impl
            // 不安全操作：
            //            1. 代码: self . as_mut_ptr ()
            //            2. 代码: self . set_len (len + count)
            fn insert_many_impl<I: Iterator<Item = T>>(&mut self, mut index: usize, iter: I) {
            let len = self.len();
            if index == len {
            return self.extend(iter);
            }
            let mut iter = iter.fuse();
            let (lower_bound, _) = iter.size_hint();
            self.reserve(lower_bound);
            let count = unsafe {
            let ptr = self.as_mut_ptr();
            let count = insert_many_batch(ptr, index, lower_bound, len, &mut iter);
            self.set_len(len + count);
            count
            };
            index += count;
            iter.enumerate().for_each(|(i, item)| self.insert(index + i, item));
            }
        }

    } // end of module group_1

    // 组 2: 通向不安全函数的路径: extend_impl
    pub mod group_2 {
        // 路径列表:
        // 2.1 pub resize -> extend -> extend_impl


        // 相关自定义类型定义:
        // 类型: SmallVec
        #[repr(C)]
        pub struct SmallVec<T, const N: usize> {
        len: TaggedLen,
        raw: RawSmallVec<T, N>,
        _marker: PhantomData<T>,
        }

        impl SmallVec {
            pub const fn new() -> SmallVec<T, N> {
            Self {
            len: TaggedLen::new(0, false, Self::is_zst()),
            raw: RawSmallVec::new(),
            _marker: PhantomData,
            }
            }
            pub fn with_capacity(capacity: usize) -> Self {
            let mut this = Self::new();
            if capacity > Self::inline_size() {
            this.grow(capacity);
            }
            this
            }
            pub fn from_vec(vec: Vec<T>) -> Self {
            if vec.capacity() == 0 {
            return Self::new();
            }
            if Self::is_zst() {
            let mut vec = vec;
            let len = vec.len();
            unsafe { vec.set_len(0) };
            Self {
            len: TaggedLen::new(len, false, Self::is_zst()),
            raw: RawSmallVec::new(),
            _marker: PhantomData,
            }
            } else {
            let mut vec = ManuallyDrop::new(vec);
            let len = vec.len();
            let cap = vec.capacity();
            let ptr = unsafe { NonNull::new_unchecked(vec.as_mut_ptr()) };
            Self {
            len: TaggedLen::new(len, true, Self::is_zst()),
            raw: RawSmallVec::new_heap(ptr, cap),
            _marker: PhantomData,
            }
            }
            }
            pub const fn from_buf(buf: [T; N]) -> Self {
            Self {
            len: TaggedLen::new(N, false, Self::is_zst()),
            raw: RawSmallVec::new_inline(MaybeUninit::new(buf)),
            _marker: PhantomData,
            }
            }
            pub fn from_buf_and_len(buf: [T; N], len: usize) -> Self {
            assert!(len <= N);
            let mut vec = Self {
            len: TaggedLen::new(len, false, Self::is_zst()),
            raw: RawSmallVec::new_inline(MaybeUninit::new(buf)),
            _marker: PhantomData,
            };
            unsafe {
            let remainder_ptr = vec.raw.as_mut_ptr_inline().add(len);
            let remainder_len = N - len;
            core::ptr::drop_in_place(
            core::ptr::slice_from_raw_parts_mut(remainder_ptr, remainder_len),
            );
            }
            vec
            }
            pub fn split_off(&mut self, at: usize) -> Self {
            let len = self.len();
            assert!(at <= len);
            let other_len = len - at;
            let mut other = Self::with_capacity(other_len);
            unsafe {
            self.set_len(at);
            other.set_len(other_len);
            core::ptr::copy_nonoverlapping(
            self.as_ptr().add(at),
            other.as_mut_ptr(),
            other_len,
            );
            }
            other
            }
            fn default() -> Self {
            Self::new()
            }
            pub fn from_slice(slice: &[T]) -> Self {
            let len = slice.len();
            if len <= Self::inline_size() {
            let mut this = Self::new();
            unsafe {
            let ptr = this.raw.as_mut_ptr_inline();
            copy_nonoverlapping(slice.as_ptr(), ptr, len);
            this.set_len(len);
            }
            this
            } else {
            let mut this = Vec::with_capacity(len);
            unsafe {
            let ptr = this.as_mut_ptr();
            copy_nonoverlapping(slice.as_ptr(), ptr, len);
            this.set_len(len);
            }
            Self::from_vec(this)
            }
            }
            pub fn from_elem(elem: T, n: usize) -> Self {
            if n > Self::inline_size() {
            Self::from_vec(vec![elem; n])
            } else {
            let mut v = Self::new();
            unsafe {
            let ptr = v.raw.as_mut_ptr_inline();
            let mut guard = DropGuard { ptr, len: 0 };
            for i in 0..n {
            guard.len = i;
            ptr.add(i).write(elem.clone());
            }
            core::mem::forget(guard);
            v.set_len(n);
            }
            v
            }
            }
            fn from_iter<I: IntoIterator<Item = T>>(iterable: I) -> Self {
            let mut vec = Self::new();
            vec.extend_impl(iterable.into_iter());
            vec
            }
            default fn spec_from(slice: &[Self::Element]) -> Self {
            slice.iter().cloned().collect()
            }
            fn spec_from(slice: &[Self::Element]) -> Self {
            Self::from_slice(slice)
            }
            fn from(slice: &'a [T]) -> Self {
            <Self as SpecFrom>::spec_from(slice)
            }
            fn from(slice: &'a [T]) -> Self {
            slice.iter().cloned().collect()
            }
            fn from(array: [T; M]) -> Self {
            if M > N {
            Self::from(Vec::from(array))
            } else {
            let mut this = Self::new();
            debug_assert!(M <= this.capacity());
            let array = ManuallyDrop::new(array);
            unsafe {
            copy_nonoverlapping(array.as_ptr(), this.as_mut_ptr(), M);
            this.set_len(M);
            }
            this
            }
            }
            fn from(array: Vec<T>) -> Self {
            Self::from_vec(array)
            }
            fn clone(&self) -> SmallVec<T, N> {
            SmallVec::from(self.as_slice())
            }

            // 公共入口点: resize
            #[inline]
            pub fn resize(&mut self, len: usize, value: T) {
                let old_len = self.len();
                if len > old_len {
                    self.extend(core::iter::repeat(value).take(len - old_len));
                } else {
                    self.truncate(len);
                }
            }

            // 不安全实现: extend_impl
            // 不安全操作：
            //            1. 代码: self . as_mut_ptr ()
            //            2. 代码: ptr . add (len)
            //            3. 代码: ptr . add (guard . len) . write (item)
            //            4. 代码: ptr . add (guard . len)
            //            5. 代码: core :: mem :: forget (guard)
            //            6. 代码: self . set_len (len)
            //            7. 代码: heap_ptr . as_ptr ()
            fn extend_impl<I: Iterator<Item = T>>(&mut self, iter: I) {
            let mut iter = iter.fuse();
            let (lower_bound, _) = iter.size_hint();
            self.reserve(lower_bound);
            let mut capacity = self.capacity();
            let mut ptr = self.as_mut_ptr();
            unsafe {
            loop {
            let mut len = self.len();
            ptr = ptr.add(len);
            let mut guard = DropGuard { ptr, len: 0 };
            iter.by_ref()
            .take(capacity - len)
            .for_each(|item| {
            ptr.add(guard.len).write(item);
            guard.len += 1;
            });
            len += guard.len;
            core::mem::forget(guard);
            self.set_len(len);
            if let Some(item) = iter.next() {
            self.push(item);
            } else {
            return;
            }
            let (heap_ptr, heap_capacity) = self.raw.heap;
            ptr = heap_ptr.as_ptr();
            capacity = heap_capacity;
            }
            }
            }

            // 中间函数: extend
            fn extend<I: IntoIterator<Item = T>>(&mut self, iterable: I) {
            self.extend_impl(iterable.into_iter());
            }
        }

    } // end of module group_2

    // 组 3: 通向不安全函数的路径: into_iter
    pub mod group_3 {
        // 路径列表:
        // 3.1 pub insert_many -> into_iter
        // 3.2 pub resize -> extend -> into_iter


        // 相关自定义类型定义:
        // 类型: SmallVec
        #[repr(C)]
        pub struct SmallVec<T, const N: usize> {
        len: TaggedLen,
        raw: RawSmallVec<T, N>,
        _marker: PhantomData<T>,
        }

        impl SmallVec {
            pub const fn new() -> SmallVec<T, N> {
            Self {
            len: TaggedLen::new(0, false, Self::is_zst()),
            raw: RawSmallVec::new(),
            _marker: PhantomData,
            }
            }
            pub fn with_capacity(capacity: usize) -> Self {
            let mut this = Self::new();
            if capacity > Self::inline_size() {
            this.grow(capacity);
            }
            this
            }
            pub fn from_vec(vec: Vec<T>) -> Self {
            if vec.capacity() == 0 {
            return Self::new();
            }
            if Self::is_zst() {
            let mut vec = vec;
            let len = vec.len();
            unsafe { vec.set_len(0) };
            Self {
            len: TaggedLen::new(len, false, Self::is_zst()),
            raw: RawSmallVec::new(),
            _marker: PhantomData,
            }
            } else {
            let mut vec = ManuallyDrop::new(vec);
            let len = vec.len();
            let cap = vec.capacity();
            let ptr = unsafe { NonNull::new_unchecked(vec.as_mut_ptr()) };
            Self {
            len: TaggedLen::new(len, true, Self::is_zst()),
            raw: RawSmallVec::new_heap(ptr, cap),
            _marker: PhantomData,
            }
            }
            }
            pub const fn from_buf(buf: [T; N]) -> Self {
            Self {
            len: TaggedLen::new(N, false, Self::is_zst()),
            raw: RawSmallVec::new_inline(MaybeUninit::new(buf)),
            _marker: PhantomData,
            }
            }
            pub fn from_buf_and_len(buf: [T; N], len: usize) -> Self {
            assert!(len <= N);
            let mut vec = Self {
            len: TaggedLen::new(len, false, Self::is_zst()),
            raw: RawSmallVec::new_inline(MaybeUninit::new(buf)),
            _marker: PhantomData,
            };
            unsafe {
            let remainder_ptr = vec.raw.as_mut_ptr_inline().add(len);
            let remainder_len = N - len;
            core::ptr::drop_in_place(
            core::ptr::slice_from_raw_parts_mut(remainder_ptr, remainder_len),
            );
            }
            vec
            }
            pub fn split_off(&mut self, at: usize) -> Self {
            let len = self.len();
            assert!(at <= len);
            let other_len = len - at;
            let mut other = Self::with_capacity(other_len);
            unsafe {
            self.set_len(at);
            other.set_len(other_len);
            core::ptr::copy_nonoverlapping(
            self.as_ptr().add(at),
            other.as_mut_ptr(),
            other_len,
            );
            }
            other
            }
            fn default() -> Self {
            Self::new()
            }
            pub fn from_slice(slice: &[T]) -> Self {
            let len = slice.len();
            if len <= Self::inline_size() {
            let mut this = Self::new();
            unsafe {
            let ptr = this.raw.as_mut_ptr_inline();
            copy_nonoverlapping(slice.as_ptr(), ptr, len);
            this.set_len(len);
            }
            this
            } else {
            let mut this = Vec::with_capacity(len);
            unsafe {
            let ptr = this.as_mut_ptr();
            copy_nonoverlapping(slice.as_ptr(), ptr, len);
            this.set_len(len);
            }
            Self::from_vec(this)
            }
            }
            pub fn from_elem(elem: T, n: usize) -> Self {
            if n > Self::inline_size() {
            Self::from_vec(vec![elem; n])
            } else {
            let mut v = Self::new();
            unsafe {
            let ptr = v.raw.as_mut_ptr_inline();
            let mut guard = DropGuard { ptr, len: 0 };
            for i in 0..n {
            guard.len = i;
            ptr.add(i).write(elem.clone());
            }
            core::mem::forget(guard);
            v.set_len(n);
            }
            v
            }
            }
            fn from_iter<I: IntoIterator<Item = T>>(iterable: I) -> Self {
            let mut vec = Self::new();
            vec.extend_impl(iterable.into_iter());
            vec
            }
            default fn spec_from(slice: &[Self::Element]) -> Self {
            slice.iter().cloned().collect()
            }
            fn spec_from(slice: &[Self::Element]) -> Self {
            Self::from_slice(slice)
            }
            fn from(slice: &'a [T]) -> Self {
            <Self as SpecFrom>::spec_from(slice)
            }
            fn from(slice: &'a [T]) -> Self {
            slice.iter().cloned().collect()
            }
            fn from(array: [T; M]) -> Self {
            if M > N {
            Self::from(Vec::from(array))
            } else {
            let mut this = Self::new();
            debug_assert!(M <= this.capacity());
            let array = ManuallyDrop::new(array);
            unsafe {
            copy_nonoverlapping(array.as_ptr(), this.as_mut_ptr(), M);
            this.set_len(M);
            }
            this
            }
            }
            fn from(array: Vec<T>) -> Self {
            Self::from_vec(array)
            }
            fn clone(&self) -> SmallVec<T, N> {
            SmallVec::from(self.as_slice())
            }

            // 公共入口点: insert_many
            pub fn insert_many<I: IntoIterator<Item = T>>(&mut self, index: usize, iterable: I) {
            self.insert_many_impl(index, iterable.into_iter());
            }

            // 公共入口点: resize
            #[inline]
            pub fn resize(&mut self, len: usize, value: T) {
                let old_len = self.len();
                if len > old_len {
                    self.extend(core::iter::repeat(value).take(len - old_len));
                } else {
                    self.truncate(len);
                }
            }

            // 不安全实现: into_iter
            // 不安全操作：
            //            1. 代码: (& this . raw as * const RawSmallVec < T , N >) . read ()
            fn into_iter(self) -> Self::IntoIter {
                unsafe {
                    let this = ManuallyDrop::new(self);
                    IntoIter {
                        raw: (&this.raw as *const RawSmallVec<T, N>).read(),
                        begin: 0,
                        end: this.len,
                        _marker: PhantomData,
                    }
                }
            }

            // 中间函数: extend
            fn extend<I: IntoIterator<Item = T>>(&mut self, iterable: I) {
            self.extend_impl(iterable.into_iter());
            }
        }

    } // end of module group_3
} // end of module lib

