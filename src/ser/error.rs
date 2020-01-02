use std::error::Error as StdError;
use std::fmt;

#[derive(Debug)]
pub enum Error {
    Unimplemented(&'static str),
    MissingTag,
    UnknownLen,
    Custom(String),
}
pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Unimplemented(name) => write!(fmt, "serialization of {} not implemented", name),
            Error::MissingTag => write!(fmt, "{}", self.description()),
            Error::UnknownLen => write!(fmt, "{}", self.description()),
            Error::Custom(msg) => msg.fmt(fmt),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match self {
            Error::Unimplemented(_) => "unimplemented serialization of type",
            Error::MissingTag => "tag is missing, only structs can be serialized",
            Error::UnknownLen => "cannot pack a sequence of unknown len",
            Error::Custom(msg) => msg,
        }
    }
}

impl serde::ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::Custom(msg.to_string())
    }
}
