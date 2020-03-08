use libcommon_sys as sys;
use std::future::Future;
use std::task::{Context, Poll, Waker};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use crate::el;

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

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use futures::executor::LocalPool;
    use futures::task::LocalSpawnExt;
    use crate::el;
    use crate::el::Element;

    thread_local!{
        static GUARD: RefCell<bool> = RefCell::new(false);
    }

    #[test]
    fn test_timer() {
        GUARD.with(|g| {
            g.replace_with(|&mut _g| false);

            let mut pool = LocalPool::new();
            let spawner = pool.spawner();

            spawner.spawn_local(async {
                let mut blocker = el::Blocker::new();

                super::Timer::new(10, 0).await;
                GUARD.with(|g| {
                    g.replace_with(|&mut _g| true);
                });

                blocker.unregister();
            }).unwrap();

            loop {
                pool.run_until_stalled();
                if !el::el_has_pending_events() {
                    break;
                }
                el::el_loop_timeout(1);
            }
            assert!(*g.borrow());
        });
    }
}
