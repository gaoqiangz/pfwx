use std::{
    ffi::c_void, future::Future, pin::Pin, task::{Context, Poll}
};
use tokio::sync::oneshot;
use windows::{
    core::Error as WinError, Win32::{
        Foundation::{BOOLEAN, HANDLE, INVALID_HANDLE_VALUE}, System::{
            Threading::{
                RegisterWaitForSingleObject, UnregisterWaitEx, WT_EXECUTEINWAITTHREAD, WT_EXECUTEONLYONCE
            }, WindowsProgramming::INFINITE
        }
    }
};

/// Win32事件句柄
pub struct Win32Event {
    handle: HANDLE,
    waiting: Option<Waiting>
}

impl Win32Event {
    /// 从`Win32 HANDLE`创建
    pub fn from_raw(handle: isize) -> Self {
        Win32Event {
            handle: HANDLE(handle),
            waiting: None
        }
    }
}

unsafe impl Sync for Win32Event {}
unsafe impl Send for Win32Event {}

/// 事件等待状态
struct Waiting {
    wait_object: HANDLE,
    tx: *mut Option<oneshot::Sender<()>>,
    rx: oneshot::Receiver<()>
}

impl Waiting {
    unsafe extern "system" fn callback(ptr: *mut c_void, _timer_fired: BOOLEAN) {
        let tx = &mut *(ptr as *mut Option<oneshot::Sender<()>>);
        tx.take().unwrap().send(()).unwrap();
    }
}

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
                //注册事件监视
                if RegisterWaitForSingleObject(
                    &mut wait_object as *mut HANDLE,
                    this.handle,
                    Some(Waiting::callback),
                    Some(tx as *mut c_void),
                    INFINITE,
                    WT_EXECUTEINWAITTHREAD | WT_EXECUTEONLYONCE
                ) == false
                {
                    let err = WinError::from_win32();
                    //注册失败释放内存(在`WinError::from_win32`后面，避免意外污染`GetLastError`)
                    drop(Box::from_raw(tx));
                    return Poll::Ready(Err(err));
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

impl Drop for Waiting {
    fn drop(&mut self) {
        unsafe {
            if UnregisterWaitEx(self.wait_object, INVALID_HANDLE_VALUE) == false {
                panic!("UnregisterWaitEx failed: {}", WinError::from_win32());
            }
            drop(Box::from_raw(self.tx));
        }
    }
}
