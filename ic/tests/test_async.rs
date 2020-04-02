use ic::error;
use ic::ic_async::{Channel, Client, RpcRegister, Server, QueryFuture};
use ic::types::{ModRpc, Rpc};
use libcommon_el as el;
use libcommon_ic as ic;
use libcommon_sys as sys;
use serde_iop::{Deserialize, Serialize};

// {{{ Hello RPC definition

#[derive(Serialize, Deserialize)]
struct HelloArg {
    value: u32,
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
        arg: HelloArg
    ) -> QueryFuture<Hello>
    {
        QueryFuture::new(ic, arg, Self::CMD, Self::ASYNC)
    }
}

// }}}

#[test]
fn test_server_client() {
    unsafe {
        sys::module_require(sys::ic_get_module(), std::ptr::null_mut());
    }

    let mut reg = RpcRegister::new();

    Module::implement(&mut reg, |arg| {
        Ok(HelloRes { result: arg.value + 23 })
    });

    el::exec_test_async(async {
        let _server = Server::new("127.0.0.1", Some(reg));

        let mut client = Client::new(None);
        let connected = client.ic.connect_once("127.0.0.1").await;
        assert!(connected);

        let res = Module::call(&mut client.ic, HelloArg { value: 30 }).await.unwrap();
        assert!(res.result == 53);
    });

    unsafe {
        sys::module_release(sys::ic_get_module());
    }
}
