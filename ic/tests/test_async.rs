use ic::error;
use ic::ic::{Client, RpcRegister, Server};
use ic::types::Rpc;
use libcommon_el as el;
use libcommon_ic as ic;
use serde_iop::{Deserialize, Serialize};
use std::rc::Rc;

// {{{ Hello RPC definition

// SayHello RPC on server

#[derive(Serialize, Deserialize)]
pub struct SayHelloArg {
    user_id: u32,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct SayHelloRes {
    result: String,
}
pub struct SayHello {}

impl Rpc for SayHello {
    type Input = SayHelloArg;
    type Output = SayHelloRes;
    type Exception = GetUserExn;

    const TAG: u16 = 1;
    const ASYNC: bool = false;
}

// GetUser RPC on client

#[derive(Serialize, Deserialize)]
pub struct GetUserArg {
    user_id: u32,
}
#[derive(Serialize, Deserialize)]
pub struct GetUserRes {
    firstname: String,
    lastname: String,
    middlename: Option<String>,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct GetUserExn {
    error: String,
}
pub struct GetUser {}

impl Rpc for GetUser {
    type Input = GetUserArg;
    type Output = GetUserRes;
    type Exception = GetUserExn;

    const TAG: u16 = 2;
    const ASYNC: bool = false;
}

pub mod iop_module {
    pub const IFACE: u16 = 1;
}

// }}}

#[test]
fn test_server_client() {
    use iop_module::IFACE;

    let _m = ic::use_module();

    let mut server_reg = RpcRegister::new();
    SayHello::implement(&mut server_reg, IFACE, |mut ic, arg| async move {
        let user = GetUser::call(
            &mut ic,
            IFACE,
            GetUserArg {
                user_id: arg.user_id,
            },
        )
        .await?;

        let result = match user.middlename {
            Some(mname) => format!("Hi, {} `{}` {}.", user.firstname, mname, user.lastname),
            None => format!("Hi, {} {}.", user.firstname, user.lastname),
        };

        Ok(SayHelloRes { result })
    });

    let mut client_reg = RpcRegister::new();
    GetUser::implement(&mut client_reg, IFACE, |_ic, arg| async move {
        match arg.user_id {
            0 => Ok(GetUserRes {
                firstname: "Joseph".to_owned(),
                middlename: Some("JoJo".to_owned()),
                lastname: "Joestar".to_owned(),
            }),
            1 => Ok(GetUserRes {
                firstname: "Gyro".to_owned(),
                middlename: None,
                lastname: "Zeppeli".to_owned(),
            }),
            _ => Err(error::Error::Exn(GetUserExn {
                error: format!("unknown user with id {}", arg.user_id),
            })),
        }
    });

    el::exec_test_async(async {
        let _server = Server::new("127.0.0.1", Some(server_reg));

        let client_reg = Rc::new(client_reg);
        let mut client = Client::new(Some(&client_reg));
        let connected = client.connect_once("127.0.0.1").await;
        assert!(connected);

        let mut channel = client.get_channel();

        let res = SayHello::call(&mut channel, IFACE, SayHelloArg { user_id: 0 })
            .await
            .unwrap();
        assert!(res.result == "Hi, Joseph `JoJo` Joestar.");

        let res = SayHello::call(&mut channel, IFACE, SayHelloArg { user_id: 1 })
            .await
            .unwrap();
        assert!(res.result == "Hi, Gyro Zeppeli.");

        let res = SayHello::call(&mut channel, IFACE, SayHelloArg { user_id: 2 })
            .await
            .unwrap_err();
        match res {
            error::Error::Exn(v) => assert!(v.error == "unknown user with id 2"),
            _ => assert!(false),
        };
    });
}
