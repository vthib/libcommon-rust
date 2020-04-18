use crate::error;
use crate::ic::Channel;
use crate::types::Rpc;
use libcommon_sys as sys;
use serde_iop::{from_bytes, to_bytes};
use std::marker::PhantomData;
use std::os::raw::{c_uchar, c_void};

// {{{ Msg

pub struct Msg<T>
where
    T: Rpc,
{
    msg: *mut sys::ic_msg_t,

    _cb: PhantomData<BoxCb<T>>,
}

type BoxCb<T> = Box<dyn FnOnce(&mut Channel, Result<<T as Rpc>::Output, error::Error>)>;

impl<T> Msg<T>
where
    T: Rpc,
{
    // Allocate enough memory to store the Rust cb */
    pub fn new(iface_tag: u16) -> Self
    {
        let msg = unsafe {
            let msg = sys::ic_msg_new(std::mem::size_of::<BoxCb<T>>() as i32);
            (*msg).cb2 = Some(Self::msg_cb);
            msg
        };

        unsafe {
            (*msg).set_async(T::ASYNC);
            (*msg).cmd = T::get_cmd(iface_tag);
        }
        Self {
            msg,
            _cb: PhantomData,
        }
    }

    pub fn set_data(&mut self, input: T::Input) {
        let mut data = Vec::new();

        data.resize(12, 0);
        data.extend_from_slice(&to_bytes(&input).unwrap());

        let mut data = data.into_boxed_slice();

        unsafe {
            (*self.msg).dlen = data.len() as u32;
            (*self.msg).data = data.as_mut_ptr() as *mut c_void;
        }
        std::mem::forget(data);
    }

    pub fn set_cb<F>(&mut self, fun: F)
    where
        F: FnOnce(&mut Channel, Result<T::Output, error::Error>) + 'static,
    {
        let cb: BoxCb<T> = Box::new(fun);
        unsafe {
            std::ptr::copy_nonoverlapping(&cb, (*self.msg).priv_.as_mut_ptr() as *mut BoxCb<T>, 1);
        }
    }

    pub fn send(&mut self, ic: &mut Channel) {
        unsafe {
            sys::__ic_query(ic.to_raw(), self.msg);
        }
    }

    extern "C" fn msg_cb(
        ic: *mut sys::ichannel_t,
        msg: *mut sys::ic_msg_t,
        status: sys::ic_status_t,
        res: *const c_uchar,
        rlen: u32,
        _exn: *const c_uchar,
        _elen: u32,
    ) {
        let res = match status {
            sys::ic_status_t_IC_MSG_OK => {
                let bytes = unsafe { std::slice::from_raw_parts(res, rlen as usize) };
                match from_bytes::<T::Output>(bytes) {
                    Ok(v) => Ok(v),
                    Err(e) => Err(error::Error::Generic(format!("unpacking error: {}", e))),
                }
            }
            _ => Err(error::Error::from(status)),
        };

        let cb: BoxCb<T> = unsafe { std::ptr::read((*msg).priv_.as_mut_ptr() as *mut BoxCb<T>) };
        cb(Channel::from_raw(ic), res);
    }
}

// }}}
// {{{ ReplyMsg

pub struct ReplyMsg {
    msg: *mut sys::ic_msg_t,
}

impl ReplyMsg {
    pub fn new(ic: &mut Channel, slot: u64, status: sys::ic_status_t) -> Self {
        let mut ic = ic.to_raw();

        let msg = unsafe { sys::ic_msg_new_for_reply(&mut ic as *mut _, slot, status as i32) };

        Self { msg }
    }

    pub fn set_data(&mut self, input: &[u8]) {
        let mut data = Vec::new();

        data.resize(12, 0);
        data.extend_from_slice(input);

        let mut data = data.into_boxed_slice();

        unsafe {
            (*self.msg).dlen = data.len() as u32;
            (*self.msg).data = data.as_mut_ptr() as *mut c_void;
        }
        std::mem::forget(data);
    }

    pub fn send(&mut self, ic: &mut Channel) {
        unsafe {
            sys::ic_queue_for_reply(ic.to_raw(), self.msg);
        }
    }
}

// }}}
