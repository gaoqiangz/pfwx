use super::{AliveWatch, Runtime, UnsafeBox};
use pbni::{pbx::Session, pbx_throw};
use std::{
    cell::RefCell, mem, panic::{self, UnwindSafe}, rc::Rc, sync::{
        atomic::{AtomicUsize, Ordering}, Mutex
    }
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
                current.replace(SyncContext::new(unsafe { pbsession.clone() }));
            }
            current.as_ref().unwrap().clone()
        })
    }

    /// 消息派发器
    pub fn dispatcher(&self) -> MessageDispatcher {
        MessageDispatcher {
            hwnd: self.inner.hwnd
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
            SetWindowLongPtrA(hwnd, GWL_USERDATA, inner.as_ref() as *const SyncContextInner as i32);

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
            let pack = Box::into_inner(Box::<MessagePack>::from_raw(mem::transmute(lparam)));
            pack.tx.send(()).unwrap(); //接收
            match pack.payload {
                MessagePayload::Invoke(payload) => {
                    if let Err(e) = panic::catch_unwind(|| {
                        (payload.handler)(payload.param, payload.alive.is_alive());
                    }) {
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
                            ctx.pbsession,
                            "{}\r\nbacktrace:\r\n{:?}",
                            panic_info,
                            backtrace::Backtrace::new()
                        );
                    }
                },
                MessagePayload::Panic(payload) => {
                    pbx_throw!(ctx.pbsession, "{}", payload.info);
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
                //FIXME
                //销毁运行时
                Runtime::drop_global();
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
    Invoke(MessagePayloadInvoke),
    Panic(MessagePayloadPanic)
}

/// 消息内容-回调过程
struct MessagePayloadInvoke {
    param: UnsafeBox<()>,
    handler: Box<dyn FnOnce(UnsafeBox<()>, bool) + Send + UnwindSafe + 'static>,
    alive: AliveWatch
}

/// 消息内容-执行异常
struct MessagePayloadPanic {
    info: String
}

/// 消息派发器
pub struct MessageDispatcher {
    hwnd: HWND
}

impl MessageDispatcher {
    /// 派发回调请求给UI线程
    ///
    /// # Safety
    ///
    /// 所有参数均为UI线程独占资源
    pub(super) async fn dispatch_invoke(
        &self,
        param: UnsafeBox<()>,
        handler: Box<dyn FnOnce(UnsafeBox<()>, bool) + Send + UnwindSafe + 'static>,
        alive: AliveWatch
    ) {
        //检查目标对象存活
        if alive.is_dead() {
            //释放参数内存
            (handler)(param, false);
            return;
        }

        self.dispatch(MessagePayload::Invoke(MessagePayloadInvoke {
            param,
            handler,
            alive
        }))
        .await;
    }

    /// 派发异常信息给UI线程
    ///
    /// # Safety
    ///
    /// 所有参数均为UI线程独占资源
    pub(super) async fn dispatch_panic(&self, info: String) {
        self.dispatch(MessagePayload::Panic(MessagePayloadPanic {
            info
        }))
        .await;
    }

    /// 派发消息给UI线程
    ///
    /// # Safety
    ///
    /// 所有参数均为UI线程独占资源
    async fn dispatch(&self, payload: MessagePayload) {
        use windows::Win32::UI::WindowsAndMessaging::{IsWindow, PostMessageA};

        //参数打包
        let (tx, mut rx) = oneshot::channel();
        let msg_pack = unsafe {
            UnsafeBox::pack(MessagePack {
                payload,
                tx
            })
        };

        unsafe {
            //派发消息
            if PostMessageA(self.hwnd, WM_SYNC_CONTEXT, WPARAM(0), LPARAM(msg_pack.as_raw() as _)) == false {
                //窗口已经被销毁，说明此时目标线程已经不存在，需要释放内存
                let msg_pack = msg_pack.unpack();
                if let MessagePayload::Invoke(payload) = msg_pack.payload {
                    (payload.handler)(payload.param, false);
                }
                return;
            }
            //等待消息被接收
            loop {
                tokio::select! {
                    _ = &mut rx => break,
                    _ = time::sleep(time::Duration::from_millis(100)) => {
                        if IsWindow(self.hwnd) == false {
                            //需要再次检查信号，避免窗口销毁前接收了消息
                            if rx.try_recv().is_ok() {
                                return;
                            }
                            //窗口已经被销毁，需要释放内存
                            let msg_pack = msg_pack.unpack();
                            if let MessagePayload::Invoke(payload) = msg_pack.payload {
                                (payload.handler)(payload.param, false);
                            }
                            break;
                        }
                    }
                }
            }
        }
    }
}
