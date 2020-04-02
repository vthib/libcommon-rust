use libcommon_sys as sys;
use std::future::Future;
use std::task::{Context, Poll, Waker};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use crate::el;
use futures::executor::LocalPool;
use futures::task::LocalSpawnExt;

// {{{ Timer

pub struct TimerState {
    fired: bool,
    waker: Option<Waker>,
}

pub struct Timer {
    state: Arc<Mutex<TimerState>>,
}

impl Future for Timer {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output>
    {
        let mut state = self.state.lock().unwrap();
        if state.fired  {
            Poll::Ready(())
        } else {
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl Timer {
    pub async fn new(next: i64, flags: sys::ev_timer_flags_t) -> Self
    {
        let state = TimerState {
            fired: false,
            waker: None,
        };
        let state = Arc::new(Mutex::new(state));

        {
            let state = state.clone();
            el::Timer::new(next, 0, flags, move |_t| {
                let mut state = state.lock().unwrap();
                state.fired = true;
                if let Some(waker) = state.waker.take() {
                    waker.wake();
                }
            });
        }
        Timer {
            state
        }
    }
}

// }}}

pub fn exec_test_async<F>(fun: F)
    where F: Future<Output = ()> + 'static
{
    let mut pool = LocalPool::new();
    let spawner = pool.spawner();

    spawner.spawn_local(fun).unwrap();

    loop {
        if pool.try_run_one() {
            break;
        }
        el::el_loop_timeout(1);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    thread_local!{
        static GUARD: RefCell<bool> = RefCell::new(false);
    }

    #[test]
    fn test_timer() {
        GUARD.with(|g| {
            g.replace_with(|&mut _g| false);
        });

        super::exec_test_async(async {
            super::Timer::new(10, 0).await;
            GUARD.with(|g| {
                g.replace_with(|&mut _g| true);
            });
        });

        GUARD.with(|g| {
            assert!(*g.borrow());
        });
    }
}
