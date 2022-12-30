//! 后台异步运行时服务
//!

use std::panic::UnwindSafe;

mod context;
mod runtime;
mod handler;

pub use handler::{CancelHandle, Handler, HandlerState};

/// 非类型安全的堆分配器
#[repr(transparent)]
struct UnsafeBox<T>(*mut T);

impl<T> UnsafeBox<T> {
    unsafe fn from_raw(raw: *mut T) -> Self { UnsafeBox(raw) }
    unsafe fn into_raw(self) -> *mut T { self.0 }
    unsafe fn pack(rhs: T) -> Self { UnsafeBox(Box::into_raw(Box::new(rhs)) as _) }
    unsafe fn unpack(self) -> T { unsafe { Box::into_inner(Box::from_raw(self.0)) } }
    unsafe fn cast<U>(self) -> UnsafeBox<U> { UnsafeBox(self.0 as *mut U) }
    fn as_raw(&self) -> *mut T { self.0 }
}

unsafe impl<T: Send> Send for UnsafeBox<T> {}
impl<T: UnwindSafe> UnwindSafe for UnsafeBox<T> {}

/// 非线程安全的指针，使其可以在线程间传递
///
/// # Safety
///
/// **确保指针在归属线程中使用**
#[repr(transparent)]
struct UnsafePointer<T>(*mut T);

impl<T> UnsafePointer<T> {
    unsafe fn from_raw(raw: *mut T) -> Self { UnsafePointer(raw) }
    unsafe fn into_raw(self) -> *mut T { self.0 }
    unsafe fn cast<U>(self) -> UnsafePointer<U> { UnsafePointer(self.0 as *mut U) }
    unsafe fn clone(&self) -> UnsafePointer<T> { UnsafePointer(self.0) }
    fn as_raw(&self) -> *mut T { self.0 }
}

unsafe impl<T> Send for UnsafePointer<T> {}
unsafe impl<T> Sync for UnsafePointer<T> {}
impl<T: UnwindSafe> UnwindSafe for UnsafePointer<T> {}
