use super::mem::UnsafeBox;
use pbni::{
    pbx::{AliveState, Session}, pbx_throw
};
use std::{
    cell::RefCell, mem, panic::{self, AssertUnwindSafe}, rc::Rc, sync::{
        atomic::{AtomicUsize, Ordering}, Mutex
    }, thread
};
use tokio::{sync::oneshot, time};
use windows::{
    core::{s, PCSTR}, Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM}, System::LibraryLoader::GetModuleHandleA, UI::WindowsAndMessaging::WM_USER
    }
};

thread_local! {
static CURRENT_CONTEXT: RefCell<Option<SyncContext>> = RefCell::new(None);
}
static CONTEXT_COUNT: AtomicUsize = AtomicUsize::new(0);
static WINDOW_CLASS_ATOM: Mutex<u16> = Mutex::new(0);
const WM_SYNC_CONTEXT: u32 = WM_USER + 0xff00;

/// UI线程同步上下文
#[derive(Clone)]
pub struct SyncContext {
    inner: Rc<SyncContextInner>
}

impl SyncContext {
    /// 获取当前线程绑定的同步上下文
    pub fn current(pbsession: &Session) -> SyncContext {
        CURRENT_CONTEXT.with(|current| {
            let mut current = current.borrow_mut();
            if current.is_none() {
                current.replace(SyncContext::new(pbsession.clone()));
            }
            current.as_ref().unwrap().clone()
        })
    }

    /// 消息派发器
    pub fn dispatcher(&self) -> Dispatcher { Dispatcher::new(self.inner.hwnd) }

    /// 处理消息
    pub fn process_message(&self) {
        use windows::Win32::UI::WindowsAndMessaging::{
            DispatchMessageA, PeekMessageA, TranslateMessage, MSG, PM_REMOVE
        };

        unsafe {
            let mut msg = MSG::default();
            if PeekMessageA(&mut msg, self.inner.hwnd, WM_SYNC_CONTEXT, WM_SYNC_CONTEXT, PM_REMOVE) == true {
                TranslateMessage(&mut msg);
                DispatchMessageA(&msg);
            }
        }
    }

    //创建UI线程同步上下文
    fn new(pbsession: Session) -> SyncContext {
        use windows::{
            core::Error as WinError, Win32::{
                Foundation::*, UI::WindowsAndMessaging::{
                    CreateWindowExA, RegisterClassA, SetWindowLongPtrA, GWL_USERDATA, HMENU, HWND_MESSAGE, WINDOW_EX_STYLE, WNDCLASSA, WS_POPUP
                }
            }
        };

        unsafe {
            let hinst = GetModuleHandleA(PCSTR::null()).unwrap_or_default();
            let mut atom = WINDOW_CLASS_ATOM.lock().unwrap();
            //注册窗口类
            if *atom == 0 {
                let mut cls: WNDCLASSA = mem::zeroed();
                cls.lpfnWndProc = Some(Self::wnd_proc);
                cls.hInstance = hinst;
                cls.lpszClassName = s!("pfwxWindowSyncCtx");
                *atom = RegisterClassA(&cls);
                if *atom == 0 {
                    panic!("RegisterClass failed: {:?}", WinError::from_win32());
                }
            }
            //创建后台消息窗口
            let hwnd = CreateWindowExA(
                WINDOW_EX_STYLE::default(),
                PCSTR::from_raw(*atom as _),
                PCSTR::null(),
                WS_POPUP,
                0,
                0,
                0,
                0,
                HWND_MESSAGE, //message-only
                HMENU::default(),
                hinst,
                None
            );
            if hwnd == HWND::default() {
                panic!("CreateWindowEx failed: {:?}", WinError::from_win32());
            }
            //计数
            CONTEXT_COUNT.fetch_add(1, Ordering::Relaxed);

            let inner = Rc::new(SyncContextInner {
                hwnd,
                pbsession
            });
            //绑定上下文
            SetWindowLongPtrA(hwnd, GWL_USERDATA, inner.as_ref() as *const SyncContextInner as _);

            SyncContext {
                inner
            }
        }
    }

    /// 窗口过程
    unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        use windows::Win32::UI::WindowsAndMessaging::{DefWindowProcA, GetWindowLongPtrA, GWL_USERDATA};

        if msg == WM_SYNC_CONTEXT {
            let ctx = &*(GetWindowLongPtrA(hwnd, GWL_USERDATA) as *const SyncContextInner);
            let session = ctx.pbsession.clone();
            let pack: MessagePack = UnsafeBox::from_raw(mem::transmute(lparam)).unpack();
            let has_rx = pack.tx.send(()).is_ok(); //接收
            match pack.payload {
                MessagePayload::Invoke(payload) => {
                    if let Err(e) = panic::catch_unwind(AssertUnwindSafe(|| {
                        (payload.handler)(payload.param, payload.alive.is_alive() && has_rx);
                    })) {
                        let panic_info = match e.downcast_ref::<String>() {
                            Some(e) => &e,
                            None => {
                                match e.downcast_ref::<&'static str>() {
                                    Some(e) => e,
                                    None => "unknown"
                                }
                            },
                        };
                        pbx_throw!(
                            session,
                            "{}\r\nbacktrace:\r\n{:?}",
                            panic_info,
                            backtrace::Backtrace::new()
                        );
                    }
                },
                MessagePayload::Panic(payload) => {
                    pbx_throw!(session, "{}", payload.info);
                }
            }
            return LRESULT(0);
        }

        DefWindowProcA(hwnd, msg, wparam, lparam)
    }
}

struct SyncContextInner {
    hwnd: HWND,
    pbsession: Session
}

impl Drop for SyncContextInner {
    fn drop(&mut self) {
        use windows::Win32::UI::WindowsAndMessaging::{DestroyWindow, UnregisterClassA};

        unsafe {
            //销毁窗口
            DestroyWindow(self.hwnd);
            if CONTEXT_COUNT.fetch_sub(1, Ordering::Relaxed) == 1 {
                //注销窗口类
                let mut atom = WINDOW_CLASS_ATOM.lock().unwrap();
                if *atom != 0 {
                    UnregisterClassA(
                        PCSTR::from_raw(*atom as _),
                        GetModuleHandleA(PCSTR::null()).unwrap_or_default()
                    );
                    *atom = 0;
                }
            }
        }
    }
}

/// 消息参数包
struct MessagePack {
    payload: MessagePayload,
    tx: oneshot::Sender<()>
}

/// 消息内容
enum MessagePayload {
    Invoke(PayloadInvoke),
    Panic(PayloadPanic)
}

/// 消息内容-回调过程
struct PayloadInvoke {
    param: UnsafeBox<()>,
    handler: Box<dyn FnOnce(UnsafeBox<()>, bool) + Send + 'static>,
    alive: AliveState
}

/// 消息内容-执行异常
struct PayloadPanic {
    info: String
}

/// 消息派发器
#[derive(Clone)]
pub struct Dispatcher {
    hwnd: HWND
}

impl Dispatcher {
    fn new(hwnd: HWND) -> Dispatcher {
        Dispatcher {
            hwnd
        }
    }

    /// 派发回调请求给UI线程执行
    pub async fn dispatch_invoke(
        &self,
        param: UnsafeBox<()>,
        handler: Box<dyn FnOnce(UnsafeBox<()>, bool) + Send + 'static>,
        alive: AliveState
    ) -> bool {
        self.dispatch(MessagePayload::Invoke(PayloadInvoke {
            param,
            handler,
            alive
        }))
        .await
    }

    /// 派发异常信息给UI线程
    pub async fn dispatch_panic(&self, info: String) -> bool {
        self.dispatch(MessagePayload::Panic(PayloadPanic {
            info
        }))
        .await
    }

    /// 派发消息给UI线程
    async fn dispatch(&self, payload: MessagePayload) -> bool {
        use windows::Win32::UI::WindowsAndMessaging::IsWindow;

        if let Some((mut rx, alive, msg_pack)) = self.post_message(payload) {
            //等待消息被接收
            loop {
                tokio::select! {
                    _ = &mut rx => return true,
                    _ = time::sleep(time::Duration::from_millis(100)) => {
                        unsafe {
                            if alive.as_ref().map(|v|v.is_dead()).unwrap_or_default() || IsWindow(self.hwnd) == false {
                                //需要再次检查信号，避免目标销毁前接收了消息
                                if rx.try_recv().is_ok() {
                                    return true;
                                }
                                //接收目标被销毁，需要释放内存
                                let msg_pack = msg_pack.unpack();
                                if let MessagePayload::Invoke(payload) = msg_pack.payload {
                                    (payload.handler)(payload.param, false);
                                }
                                return false;
                            }
                        }
                    }
                }
            }
        } else {
            false
        }
    }

    /// 阻塞派发回调请求给UI线程执行
    ///
    /// # Description
    ///
    /// 在非异步上下文中使用
    pub fn dispatch_invoke_blocking(
        &self,
        param: UnsafeBox<()>,
        handler: Box<dyn FnOnce(UnsafeBox<()>, bool) + Send + 'static>,
        alive: AliveState
    ) -> bool {
        self.dispatch_blocking(MessagePayload::Invoke(PayloadInvoke {
            param,
            handler,
            alive
        }))
    }

    /// 阻塞派发异常信息给UI线程
    ///
    /// # Description
    ///
    /// 在非异步上下文中使用
    pub fn dispatch_panic_blocking(&self, info: String) -> bool {
        self.dispatch_blocking(MessagePayload::Panic(PayloadPanic {
            info
        }))
    }

    /// 阻塞派发消息给UI线程
    ///
    /// # Description
    ///
    /// 在非异步上下文中使用
    fn dispatch_blocking(&self, payload: MessagePayload) -> bool {
        use windows::Win32::UI::WindowsAndMessaging::IsWindow;

        if let Some((mut rx, alive, msg_pack)) = self.post_message(payload) {
            //等待消息被接收
            loop {
                if rx.try_recv().is_ok() {
                    return true;
                }
                unsafe {
                    if alive.as_ref().map(|v| v.is_dead()).unwrap_or_default() || IsWindow(self.hwnd) == false
                    {
                        //接收目标被销毁，需要释放内存
                        let msg_pack = msg_pack.unpack();
                        if let MessagePayload::Invoke(payload) = msg_pack.payload {
                            (payload.handler)(payload.param, false);
                        }
                        return false;
                    }
                }
                thread::sleep(time::Duration::from_millis(100));
            }
        } else {
            false
        }
    }

    /// 派发消息
    fn post_message(
        &self,
        payload: MessagePayload
    ) -> Option<(oneshot::Receiver<()>, Option<AliveState>, UnsafeBox<MessagePack>)> {
        use windows::Win32::UI::WindowsAndMessaging::PostMessageA;

        let alive = if let MessagePayload::Invoke(payload) = &payload {
            Some(payload.alive.clone())
        } else {
            None
        };

        //参数打包
        let (tx, rx) = oneshot::channel();
        let msg_pack = UnsafeBox::pack(MessagePack {
            payload,
            tx
        });

        unsafe {
            if PostMessageA(self.hwnd, WM_SYNC_CONTEXT, WPARAM(0), LPARAM(msg_pack.as_raw() as _)) == false {
                //窗口已经被销毁，说明此时目标线程已经不存在，需要释放内存
                let msg_pack = msg_pack.unpack();
                if let MessagePayload::Invoke(payload) = msg_pack.payload {
                    (payload.handler)(payload.param, false);
                }
                return None;
            }
        }

        Some((rx, alive, msg_pack))
    }
}
