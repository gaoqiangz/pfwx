use std::{
    ffi::c_void, future::Future, pin::Pin, task::{Context, Poll}, time::Duration
};

use tokio::sync::oneshot;
pub use windows::Win32::Foundation::HANDLE as HEVENT;
use windows::{
    core::Error as WinError, Win32::{
        Foundation::{
            CloseHandle, DuplicateHandle, DUPLICATE_SAME_ACCESS, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT
        }, System::Threading::{
            CreateEventA, GetCurrentProcess, RegisterWaitForSingleObject, ResetEvent, SetEvent, UnregisterWaitEx, WaitForSingleObject, INFINITE, WT_EXECUTEINWAITTHREAD, WT_EXECUTEONLYONCE
        }
    }
};

/// Win32事件句柄
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Win32Event {
    handle: HEVENT,
    owned: bool,
    waiting: Option<Waiting>
}

impl Win32Event {
    /// 创建自动信号类型事件
    pub fn auto() -> Self {
        let handle = unsafe { CreateEventA(None, false, false, None).expect("CreateEventA failed") };
        Win32Event {
            handle,
            owned: true,
            waiting: None
        }
    }

    /// 创建手动信号类型事件
    pub fn manual() -> Self {
        let handle = unsafe { CreateEventA(None, true, false, None).expect("CreateEventA failed") };
        Win32Event {
            handle,
            owned: true,
            waiting: None
        }
    }

    /// 从`HANDLE`创建
    pub fn from_raw(handle: HEVENT) -> Self {
        Win32Event {
            handle,
            owned: false,
            waiting: None
        }
    }

    /// 从`HANDLE`创建并拥有所有权
    pub fn take_raw(handle: HEVENT) -> Self {
        Win32Event {
            handle,
            owned: true,
            waiting: None
        }
    }

    /// 转换为`HANDLE`
    pub fn into_raw(mut self) -> HEVENT {
        self.owned = false;
        self.handle
    }

    pub fn as_raw(&self) -> HEVENT { self.handle }

    /// 设置信号
    pub fn set(&self) -> Result<(), WinError> { unsafe { SetEvent(self.handle) } }

    /// 重置信号
    pub fn reset(&self) -> Result<(), WinError> { unsafe { ResetEvent(self.handle) } }

    /// 阻塞等待信号
    pub fn blocking_wait(&self) -> Result<bool, WinError> {
        let rc = unsafe { WaitForSingleObject(self.handle, INFINITE) };
        match rc {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            _ => Err(WinError::from_win32())
        }
    }

    /// 指定超时内阻塞等待信号
    pub fn wait_timeout(&self, dur: Duration) -> Result<bool, WinError> {
        let rc = unsafe { WaitForSingleObject(self.handle, dur.as_millis() as u32) };
        match rc {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            _ => Err(WinError::from_win32())
        }
    }
}

impl Clone for Win32Event {
    fn clone(&self) -> Self {
        let handle = unsafe {
            let hprocess = GetCurrentProcess();
            let mut handle = HEVENT::default();
            DuplicateHandle(hprocess, self.handle, hprocess, &mut handle, 0, true, DUPLICATE_SAME_ACCESS)
                .expect("DuplicateHandle failed");
            handle
        };
        Win32Event {
            handle,
            owned: true,
            waiting: None
        }
    }
}

unsafe impl Sync for Win32Event {}
unsafe impl Send for Win32Event {}

impl Future for Win32Event {
    type Output = Result<(), WinError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        loop {
            if let Some(waiting) = &mut this.waiting {
                match Pin::new(&mut waiting.rx).poll(cx) {
                    Poll::Ready(Ok(())) => return Poll::Ready(Ok(())),
                    Poll::Ready(Err(_)) => unreachable!(),
                    Poll::Pending => return Poll::Pending
                }
            }

            let (tx, rx) = oneshot::channel();
            let tx = Box::into_raw(Box::new(Some(tx)));
            let mut wait_object = HANDLE::default();
            unsafe {
                // 注册事件监视
                if let Err(e) = RegisterWaitForSingleObject(
                    &mut wait_object as *mut HANDLE,
                    this.handle,
                    Some(Waiting::callback),
                    Some(tx as *mut c_void),
                    INFINITE,
                    WT_EXECUTEINWAITTHREAD | WT_EXECUTEONLYONCE
                ) {
                    drop(Box::from_raw(tx));
                    return Poll::Ready(Err(e));
                }
            }

            this.waiting = Some(Waiting {
                wait_object,
                rx,
                tx
            });
        }
    }
}

impl Drop for Win32Event {
    fn drop(&mut self) {
        if self.owned {
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}

/// 事件等待状态
struct Waiting {
    wait_object: HANDLE,
    tx: *mut Option<oneshot::Sender<()>>,
    rx: oneshot::Receiver<()>
}

impl Waiting {
    unsafe extern "system" fn callback(ptr: *mut c_void, _timer_fired: bool) {
        let tx = &mut *(ptr as *mut Option<oneshot::Sender<()>>);
        if let Some(tx) = tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for Waiting {
    fn drop(&mut self) {
        unsafe {
            UnregisterWaitEx(self.wait_object, Some(INVALID_HANDLE_VALUE)).expect("UnregisterWaitEx failed");
            drop(Box::from_raw(self.tx));
        }
    }
}
