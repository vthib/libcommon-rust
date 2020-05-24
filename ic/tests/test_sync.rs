use ic::ic_sync::{Client, RpcRegister, Server};
use ic::types_sync::Rpc;
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

    const ASYNC: bool = false;
    const TAG: u16 = 2;
}

pub mod iop_module {
    pub const IFACE: u16 = 1;
}

// }}}

#[test]
fn test_server_client() {
    use iop_module::IFACE;

    let _m = ic::use_module();

    let mut reg = RpcRegister::new();

    Hello::implement(&mut reg, IFACE, |arg| {
        Ok(HelloRes {
            result: arg.value as u32 + 23,
        })
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

        Hello::call(ic, IFACE, HelloArg { value: 30 }, |ic, res| {
            RESULT.with(|result| {
                *result.borrow_mut() = res.unwrap().result;
            });
            ic.disconnect();
        });
    });

    el::el::el_loop();

    RESULT.with(|result| assert!(*result.borrow() == 53));
}
