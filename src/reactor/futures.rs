use super::event::{Win32Event, HEVENT};
use futures_util::future::{self, Either};
use pbni::primitive::pbulong;
use std::future::Future;

/// 执行`fut`任务并支持通过Win32 Event Handle信号进行取消
///
/// # Returns
///
/// 执行完成返回`Some(Output)`，被取消返回`None`
///
/// # Panics
///
/// `hevent`为无效句柄时会触发Panic
///
/// # Undefined behaviors
///
/// `hevent`在WAIT过程中被销毁将导致**未定义行为(UB)**
pub async fn cancel_by_event<F>(fut: F, hevent: pbulong) -> Option<F::Output>
where
    F: Future
{
    let event = Win32Event::from_raw(HEVENT(hevent as _));
    tokio::pin!(fut);
    tokio::pin!(event);
    match future::select(fut, event).await {
        Either::Left((rv, _)) => Some(rv),
        Either::Right((rv, _)) => {
            match rv {
                Ok(_) => None,
                Err(e) => panic!("wait hevent failed: {e}")
            }
        },
    }
}
