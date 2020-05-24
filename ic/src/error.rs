use libcommon_sys as sys;
use std::error;
use std::fmt;

#[derive(Debug)]
pub enum Error<T> {
    Exn(T),
    Generic(String),
    Retry,
    Abort,
    Invalid,
    Unimplemented,
    ServerError,
    ProxyError,
    TimedOut,
    Canceled,
}

impl<T> fmt::Display for Error<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "query error: {}",
            match self {
                Error::Exn(_s) => "exception",
                Error::Generic(s) => s,
                Error::Retry => "retry",
                Error::Abort => "abort",
                Error::Invalid => "invalid",
                Error::Unimplemented => "unimplemented",
                Error::ServerError => "server error",
                Error::ProxyError => "proxy error",
                Error::TimedOut => "timed out",
                Error::Canceled => "canceled",
            }
        )
    }
}

impl<T> error::Error for Error<T> where T: std::fmt::Debug {}

impl<T> From<sys::ic_status_t> for Error<T> {
    fn from(status: sys::ic_status_t) -> Self {
        match status {
            sys::ic_status_t_IC_MSG_RETRY => Self::Retry,
            sys::ic_status_t_IC_MSG_ABORT => Self::Abort,
            sys::ic_status_t_IC_MSG_INVALID => Self::Invalid,
            sys::ic_status_t_IC_MSG_UNIMPLEMENTED => Self::Unimplemented,
            sys::ic_status_t_IC_MSG_SERVER_ERROR => Self::ServerError,
            sys::ic_status_t_IC_MSG_PROXY_ERROR => Self::ProxyError,
            sys::ic_status_t_IC_MSG_TIMEDOUT => Self::TimedOut,
            sys::ic_status_t_IC_MSG_CANCELED => Self::Canceled,
            _ => {
                unreachable!();
            }
        }
    }
}

impl<T> From<Error<T>> for sys::ic_status_t {
    fn from(status: Error<T>) -> Self {
        match status {
            Error::Retry => sys::ic_status_t_IC_MSG_RETRY,
            Error::Abort => sys::ic_status_t_IC_MSG_ABORT,
            Error::Invalid => sys::ic_status_t_IC_MSG_INVALID,
            Error::Unimplemented => sys::ic_status_t_IC_MSG_UNIMPLEMENTED,
            Error::ServerError => sys::ic_status_t_IC_MSG_SERVER_ERROR,
            Error::ProxyError => sys::ic_status_t_IC_MSG_PROXY_ERROR,
            Error::TimedOut => sys::ic_status_t_IC_MSG_TIMEDOUT,
            Error::Canceled => sys::ic_status_t_IC_MSG_CANCELED,
            Error::Exn(_) => sys::ic_status_t_IC_MSG_EXN,
            /* TODO: this isn't what we want to do */
            Error::Generic(s) => {
                println!("generic error: {}", s);
                sys::ic_status_t_IC_MSG_SERVER_ERROR
            }
        }
    }
}
