use crate::error;
use crate::types::{ModRpc, Rpc};
use libc;
use libcommon_sys as sys;
use serde_iop::{from_bytes, to_bytes, Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::mem;
use std::os::raw::{c_uchar, c_void};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

// {{{ RPC Implementation register

pub struct RpcRegister<'a> {
    map: sys::qm_ic_cbs_t,

    impls: HashMap<i32, Box<dyn Fn(&'a [u8]) -> Result<Vec<u8>, error::Error> + 'a>>,
}

impl<'a> RpcRegister<'a> {
    pub fn new() -> Self {
        let map = unsafe {
            let mut map: sys::qm_ic_cbs_t = mem::zeroed();

            sys::qhash_init(
                &mut map.qh,
                mem::size_of::<u32>() as u16,
                mem::size_of::<sys::ic_cb_entry_t>() as u16,
                false,
                std::ptr::null_mut(),
            );
            map
        };

        Self {
            map,
            impls: HashMap::new(),
        }
    }

    pub fn register<I, O>(&mut self, cmd: i32, fun: impl Fn(I) -> Result<O, error::Error> + 'static)
    where
        I: Deserialize<'a>,
        O: Serialize,
    {
        self.impls.insert(
            cmd,
            Box::new(move |data: &[u8]| {
                let input: I = from_bytes(data).unwrap();

                match fun(input) {
                    Ok(res) => Ok(to_bytes(&res).unwrap()),
                    Err(_e) => Err(error::Error::Generic("rpc error".to_owned())),
                }
            }),
        );

        unsafe {
            let mut entry: sys::ic_cb_entry_t = mem::zeroed();

            entry.cb_type = sys::ic_cb_entry_type_t_IC_CB_NORMAL_RAW;
            entry.u.cbr.cb = Some(RpcRegister::call_rpc_impl);

            sys::_ic_register(&mut self.map, cmd, &mut entry);
        }
    }

    unsafe extern "C" fn call_rpc_impl(
        ic: *mut sys::ichannel_t,
        slot: u64,
        cmd: i32,
        data: sys::lstr_t,
        _hdr: *const sys::ic__hdr__t,
    ) {
        let ic = Channel::from_raw(ic);

        let res = match ic.register.as_ref().and_then(|reg| reg.impls.get(&cmd)) {
            Some(cb) => {
                let data = std::slice::from_raw_parts(
                    data.__bindgen_anon_1.s as *const c_void as *const u8,
                    data.len as usize,
                );

                (cb)(&data)
            }
            None => Err(error::Error::Generic(format!(
                "unimplemented RPC with cmd {}",
                cmd
            ))),
        };
        match res {
            Ok(r) => {
                ic.reply(&r, slot, sys::ic_status_t_IC_MSG_OK);
            }
            Err(err) => {
                println!("error: {}", err);
            }
        };
    }
}

// }}}
// {{{ Helpers

unsafe fn hostname_to_su(hostname: &str) -> sys::sockunion_t {
    let mut su: sys::sockunion_t = mem::zeroed();
    let mut host: sys::pstream_t = mem::zeroed();
    let mut port: sys::in_port_t = mem::zeroed();

    let hostname: Vec<u8> = hostname.bytes().collect();
    let ps = sys::ps_init(hostname.as_ptr() as *const c_void, hostname.len());

    sys::addr_parse_minport(ps, &mut host, &mut port, 1, -1);
    sys::addr_info(&mut su, libc::AF_INET as u16, host, port);
    su
}

// }}}
// {{{ Server

struct InnerServer<'a> {
    el: sys::el_t,

    register: Option<Rc<RpcRegister<'a>>>,

    clients: Vec<Client<'a>>,
}

pub struct Server<'a> {
    _inner: Box<InnerServer<'a>>,
}

impl<'a> Server<'a> {
    pub fn new(hostname: &str, register: Option<RpcRegister<'a>>) -> Self {
        let register = match register {
            Some(r) => Some(Rc::new(r)),
            None => None,
        };

        let mut inner = Box::new(InnerServer {
            el: std::ptr::null_mut(),
            register,
            clients: Vec::new(),
        });

        inner.el = unsafe {
            let su = hostname_to_su(hostname);

            sys::ic_listento(
                &su,
                libc::SOCK_STREAM,
                libc::IPPROTO_TCP,
                &mut *inner as *mut InnerServer as *mut c_void,
                Some(Server::on_accept),
            )
        };

        Self { _inner: inner }
    }

    unsafe extern "C" fn on_accept(_ev: sys::el_t, fd: i32, data: *mut c_void) -> i32 {
        let inner: &mut InnerServer = &mut *(data as *mut InnerServer);
        let mut ic = Client::new(inner.register.as_ref());

        ic.ic.raw_ic.on_event = Some(Server::on_event);
        ic.ic.spawn(fd);

        inner.clients.push(ic);
        0
    }

    unsafe extern "C" fn on_event(_ic: *mut sys::ichannel_t, _evt: sys::ic_event_t) {}
}

impl<'a> Drop for InnerServer<'a> {
    fn drop(&mut self) {
        unsafe {
            sys::el_unregister(&mut self.el);
        }
    }
}

// }}}
// {{{ Client

pub struct Client<'a> {
    pub ic: Box<Channel<'a>>,
}

impl<'a> Client<'a> {
    pub fn new(register: Option<&Rc<RpcRegister<'a>>>) -> Self {
        let mut ic = Box::new(Channel {
            raw_ic: unsafe { mem::zeroed() },
            connect_state: None,
            register: None,
        });

        unsafe {
            sys::ic_init(&mut ic.raw_ic);

            ic.raw_ic.set_no_autodel(true);
            ic.raw_ic.priv_data = &mut *ic as *mut Channel as *mut c_void;
        };

        if let Some(reg) = register {
            ic.raw_ic.impl_ = &reg.map;
            ic.register = Some(reg.clone())
        };

        Self { ic }
    }
}

// }}}
// {{{ Channel

pub struct Channel<'a> {
    raw_ic: sys::ichannel_t,

    connect_state: Option<Arc<Mutex<ConnectState>>>,

    register: Option<Rc<RpcRegister<'a>>>,
}

impl<'a> Channel<'a> {
    pub fn from_raw<'b>(ic: *mut sys::ichannel_t) -> &'b mut Self {
        unsafe { &mut *((*ic).priv_data as *mut Self) }
    }

    pub fn to_raw(&mut self) -> *mut sys::ichannel_t {
        &mut self.raw_ic as *mut _
    }

    fn spawn(&mut self, fd: i32) {
        unsafe {
            sys::ic_spawn(&mut self.raw_ic, fd, None);
        }
    }

    pub fn connect_once(&mut self, hostname: &str) -> ConnectFuture {
        let state = Arc::new(Mutex::new(ConnectState {
            res: None,
            waker: None,
        }));

        self.connect_state = Some(state.clone());

        unsafe {
            self.raw_ic.su = hostname_to_su(hostname);
            self.raw_ic.on_event = Some(Channel::on_event);
            sys::ic_connect(&mut self.raw_ic);
        }

        ConnectFuture { state }
    }

    unsafe extern "C" fn on_event(raw_ic: *mut sys::ichannel_t, evt: sys::ic_event_t) {
        let ic = Channel::from_raw(raw_ic);

        match ic.connect_state.as_ref() {
            Some(state) => {
                let mut state = state.lock().unwrap();

                if evt == sys::ic_event_t_IC_EVT_CONNECTED {
                    state.res = Some(true);
                } else if evt == sys::ic_event_t_IC_EVT_DISCONNECTED {
                    state.res = Some(false);
                }
                if let Some(waker) = state.waker.take() {
                    waker.wake();
                }
            }
            None => return,
        };
    }

    pub fn disconnect(&mut self) {
        unsafe {
            sys::ic_disconnect(&mut self.raw_ic);
        }
    }

    pub fn query<M, T>(&mut self, input: T::Input) -> QueryFuture<T>
    where
        T: Rpc<'static>,
        M: ModRpc<'static, RPC = T>,
    {
        QueryFuture::new(self, input, M::CMD, M::ASYNC)
    }

    pub fn reply(&mut self, res: &[u8], slot: u64, status: sys::ic_status_t) {
        let mut ic = self.to_raw();
        let msg = unsafe { sys::ic_msg_new_for_reply(&mut ic as *mut _, slot, status as i32) };

        let mut data = Vec::new();
        data.resize(12, 0);
        data.extend_from_slice(res);
        let mut data = data.into_boxed_slice();

        unsafe {
            (*msg).dlen = data.len() as u32;
            (*msg).data = data.as_mut_ptr() as *mut c_void;
        }
        std::mem::forget(data);

        unsafe {
            sys::ic_queue_for_reply(ic, msg);
        }
    }
}

impl<'a> Drop for Channel<'a> {
    fn drop(&mut self) {
        unsafe {
            sys::ic_wipe(&mut self.raw_ic);
        }
    }
}

// }}}
// {{{ Query Future
//
struct QueryState<T> {
    result: Option<Result<T, error::Error>>,
    waker: Option<Waker>,
}

pub struct QueryFuture<T>
where
    T: Rpc<'static>,
{
    state: Arc<Mutex<QueryState<T::Output>>>,
}

impl<T> Future for QueryFuture<T>
where
    T: Rpc<'static>,
{
    type Output = Result<T::Output, error::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut state = self.state.lock().unwrap();
        match state.result.take() {
            Some(r) => Poll::Ready(r),
            None => {
                state.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

type MsgPayload<T> = Mutex<QueryState<T>>;

impl<T> QueryFuture<T>
where
    T: Rpc<'static>,
{
    pub fn new(ic: &mut Channel, input: T::Input, cmd: i32, async_: bool) -> Self {
        let msg = unsafe { sys::ic_msg_new(std::mem::size_of::<*const c_void>() as i32) };

        // Serialize input
        let mut data = Vec::new();
        data.resize(12, 0);
        data.extend_from_slice(&to_bytes(&input).unwrap());
        let mut data = data.into_boxed_slice();

        unsafe {
            (*msg).dlen = data.len() as u32;
            (*msg).data = data.as_mut_ptr() as *mut c_void;

            (*msg).cb2 = Some(Self::msg_cb);
            (*msg).set_async(async_);
            (*msg).cmd = cmd;
        }
        std::mem::forget(data);

        // Create state that will be shared between the future, and the query callback.
        let state = QueryState {
            result: None,
            waker: None,
        };
        let state = Arc::new(Mutex::new(state));

        /* store in the msg a clone of the arc */
        {
            let state = Arc::into_raw(state.clone());
            unsafe {
                std::ptr::copy_nonoverlapping(
                    &(state as *mut c_void),
                    (*msg).priv_.as_mut_ptr() as *mut *mut c_void,
                    1,
                );
            }
        }

        unsafe {
            sys::__ic_query(ic.to_raw(), msg);
        }

        // and return a future with the shared state
        Self { state }
    }

    extern "C" fn msg_cb(
        _ic: *mut sys::ichannel_t,
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

        let state = unsafe {
            let payload = (*msg).priv_.as_ptr() as *const *const MsgPayload<T::Output>;
            Arc::from_raw(std::ptr::read(payload))
        };

        let mut state = state.lock().unwrap();
        state.result = Some(res);
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }
}

// }}}
// {{{ Connect Future

struct ConnectState {
    res: Option<bool>,
    waker: Option<Waker>,
}

pub struct ConnectFuture {
    state: Arc<Mutex<ConnectState>>,
}

impl Future for ConnectFuture {
    type Output = bool;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let mut state = self.state.lock().unwrap();
        match state.res.take() {
            Some(r) => Poll::Ready(r),
            None => {
                state.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

// }}}
