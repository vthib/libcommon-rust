use crate::error;
use crate::msg_sync::ReplyMsg;
use libc;
use libcommon_sys as sys;
use serde_iop::{from_bytes, to_bytes, Deserialize, Serialize};
use std::collections::HashMap;
use std::mem;
use std::os::raw::c_void;
use std::rc::Rc;

// {{{ RPC Implementation register

pub struct RpcRegister {
    map: sys::qm_ic_cbs_t,

    impls: HashMap<i32, Box<dyn Fn(&[u8]) -> Result<Vec<u8>, error::Error>>>,
}

impl RpcRegister {
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
        I: for<'a> Deserialize<'a>,
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
                let mut msg = ReplyMsg::new(ic, slot, sys::ic_status_t_IC_MSG_OK);
                msg.set_data(&r);
                msg.send(ic);
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

    register: Option<Rc<RpcRegister>>,

    clients: Vec<Client<'a>>,
}

pub struct Server<'a> {
    _inner: Box<InnerServer<'a>>,
}

impl<'a> Server<'a> {
    pub fn new(hostname: &str, register: Option<RpcRegister>) -> Self {
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

    unsafe extern "C" fn on_event(_ic: *mut sys::ichannel_t, _evt: sys::ic_event_t) {
    }
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
    pub fn new(register: Option<&Rc<RpcRegister>>) -> Self {
        let mut ic = Box::new(Channel {
            raw_ic: unsafe { mem::zeroed() },
            on_event_cb: None,
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

    on_event_cb: Option<Box<dyn Fn(&mut Channel, bool) + 'a>>,

    register: Option<Rc<RpcRegister>>,
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

    pub fn connect<F>(&mut self, hostname: &str, on_event_cb: F)
    where
        F: Fn(&mut Channel, bool) + 'a,
    {
        unsafe {
            self.raw_ic.su = hostname_to_su(hostname);
            self.raw_ic.on_event = Some(Channel::on_event);
            self.on_event_cb = Some(Box::new(on_event_cb));
            sys::ic_connect(&mut self.raw_ic);
        }
    }

    unsafe extern "C" fn on_event(raw_ic: *mut sys::ichannel_t, evt: sys::ic_event_t) {
        let ic = Channel::from_raw(raw_ic);

        match ic.on_event_cb.as_ref() {
            Some(cb) => {
                let ic = Channel::from_raw(raw_ic);

                if evt == sys::ic_event_t_IC_EVT_CONNECTED {
                    (cb)(ic, true);
                } else if evt == sys::ic_event_t_IC_EVT_DISCONNECTED {
                    (cb)(ic, false);
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
}

impl<'a> Drop for Channel<'a> {
    fn drop(&mut self) {
        unsafe {
            sys::ic_wipe(&mut self.raw_ic);
        }
    }
}

// }}}
