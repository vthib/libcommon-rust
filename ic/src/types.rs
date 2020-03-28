use serde_iop::{Deserialize, Serialize};

pub trait Rpc<'a> {
    type Input: Serialize + Deserialize<'a>;
    type Output: Serialize + Deserialize<'a>;
}

pub trait ModRpc<'a> {
    type RPC: Rpc<'a>;

    const ASYNC: bool;
    const CMD: i32;
}
