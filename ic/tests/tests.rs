use ic::error;
use ic::ic::{Channel, Client, RpcRegister, Server};
use ic::msg::Msg;
use ic::types::{ModRpc, Rpc};
use libcommon_el as el;
use libcommon_el::el::Element;
use libcommon_ic as ic;
use serde_iop::{Deserialize, Serialize};
use std::cell::RefCell;

// {{{ Hello RPC definition

#[derive(Serialize, Deserialize)]
struct HelloArg {
    value: u8,
}
#[derive(Serialize, Deserialize)]
struct HelloRes {
    result: u32,
}
struct Hello {}

impl Rpc for Hello {
    type Input = HelloArg;
    type Output = HelloRes;
}

struct Module {}
impl ModRpc for Module {
    type RPC = Hello;
    const ASYNC: bool = false;
    const CMD: i32 = 2;
}

impl Module {
    fn implement<'a, F>(reg: &mut RpcRegister<'a>, fun: F)
    where
        F: Fn(HelloArg) -> Result<HelloRes, error::Error> + 'static,
    {
        reg.register(Self::CMD, fun);
    }

    fn call(
        ic: &mut Channel,
        arg: HelloArg,
        cb: impl FnOnce(&mut Channel, Result<HelloRes, error::Error>) + 'static,
    ) {
        let mut msg = Msg::new::<Self>();
        msg.set_data(arg);
        msg.set_cb(cb);
        msg.send(ic);
    }
}

// }}}

#[test]
fn test_server_client() {
    let _m = ic::use_module();

    let mut reg = RpcRegister::new();

    Module::implement(&mut reg, |arg| {
        Ok(HelloRes { result: arg.value as u32 + 23 })
    });

    thread_local! {
        static RESULT: RefCell<u32> = RefCell::new(0);
    }

    let blocker = RefCell::new(el::el::Blocker::new());

    let _server = Server::new("127.0.0.1", Some(reg));

    let mut client = Client::new(None);
    client.ic.connect("127.0.0.1", |ic, connected| {
        if !connected {
            return;
        }

        blocker.borrow_mut().unregister();

        Module::call(ic, HelloArg { value: 30 }, |ic, res| {
            RESULT.with(|result| {
                *result.borrow_mut() = res.unwrap().result;
            });
            ic.disconnect();
        });
    });

    el::el::el_loop();

    RESULT.with(|result| assert!(*result.borrow() == 53));
}
