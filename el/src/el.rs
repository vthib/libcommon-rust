use libcommon_sys as sys;
use std::os::raw::c_void;

// {{{ Element

pub trait Element {
    fn get_el(&self) -> sys::el_t;

    fn unref(&mut self) {
        unsafe {
            sys::el_unref(self.get_el());
        }
    }

    fn unregister(&mut self) {
        unsafe {
            sys::el_unregister(&mut self.get_el());
        }
    }
}

// }}}
// {{{ Timer

pub struct Timer(sys::el_t);

impl Timer {
    extern "C" fn call_cb(el: sys::el_t, data: sys::data_t) {
        let cb = unsafe { Box::from_raw(data.ptr as *mut Box<dyn FnOnce(Timer)>) };

        (cb)(Timer(el));
    }

    pub fn new<F>(next: i64, repeat: i64, flags: sys::ev_timer_flags_t, cb: F) -> Self
    where
        F: FnOnce(Timer),
        F: 'static,
    {
        let cb: Box<Box<dyn FnOnce(Timer)>> = Box::new(Box::new(cb));
        let data = sys::data_t {
            ptr: Box::into_raw(cb) as *mut c_void,
        };

        let cb_f = Timer::call_cb as unsafe extern "C" fn(sys::el_t, sys::data_t);
        let cb_f = Some(cb_f);

        unsafe {
            let el = sys::el_timer_register_d(next, repeat, flags, cb_f, data);
            Self(el)
        }
    }
}

impl Element for Timer {
    fn get_el(&self) -> sys::el_t {
        self.0
    }
}

// }}}
// {{{ Blocker

pub struct Blocker(sys::el_t);

impl Blocker {
    pub fn new() -> Self {
        unsafe { Self(sys::el_blocker_register()) }
    }
}

impl Element for Blocker {
    fn get_el(&self) -> sys::el_t {
        self.0
    }
}

// }}}
// {{{ API

pub fn el_loop() {
    unsafe { sys::el_loop() }
}

pub fn el_loop_timeout(timeout_msec: i32) {
    unsafe { sys::el_loop_timeout(timeout_msec) }
}

pub fn el_has_pending_events() -> bool {
    unsafe { sys::el_has_pending_events() }
}

// }}}

#[cfg(test)]
mod tests {
    use super::Element;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn test_timer() {
        let mut blocker = super::Blocker::new();

        let cnt = Rc::new(RefCell::new(0));
        {
            let cnt = cnt.clone();
            let _timer = super::Timer::new(10, 0, 0, move |_timer| {
                cnt.replace_with(|&mut v| v + 1);
                blocker.unregister();
            });
        }
        super::el_loop();
        assert_eq!(*cnt.borrow(), 1);
    }
}
