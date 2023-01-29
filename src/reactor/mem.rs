/// 非类型安全的堆分配器
#[repr(transparent)]
pub struct UnsafeBox<T>(*mut Option<T>);

impl<T> UnsafeBox<T> {
    pub unsafe fn from_raw(raw: *mut Option<T>) -> Self { UnsafeBox(raw) }
    pub fn into_raw(self) -> *mut Option<T> { self.0 }
    pub fn pack(rhs: T) -> Self { UnsafeBox(Box::into_raw(Box::new(Some(rhs)))) }
    pub unsafe fn unpack(self) -> T { (&mut *(Box::from_raw(self.0))).take().unwrap() }
    pub fn cast<U>(self) -> UnsafeBox<U> { UnsafeBox(self.0 as *mut Option<U>) }
    pub fn as_raw(&self) -> *mut Option<T> { self.0 }
}

unsafe impl<T: Send> Send for UnsafeBox<T> {}
unsafe impl<T: Sync> Sync for UnsafeBox<T> {}

/// 非线程安全的指针，使其可以在线程间传递
///
/// # Safety
///
/// **确保指针在归属线程中使用**
#[repr(transparent)]
pub struct UnsafePointer<T>(*mut T);

impl<T> UnsafePointer<T> {
    pub unsafe fn from_raw(raw: *mut T) -> Self { UnsafePointer(raw) }
    pub fn into_raw(self) -> *mut T { self.0 }
    pub fn cast<U>(self) -> UnsafePointer<U> { UnsafePointer(self.0 as *mut U) }
    pub fn as_raw(&self) -> *mut T { self.0 }
}

impl<T> Clone for UnsafePointer<T> {
    fn clone(&self) -> Self { UnsafePointer(self.0) }
}

unsafe impl<T> Send for UnsafePointer<T> {}
unsafe impl<T> Sync for UnsafePointer<T> {}
