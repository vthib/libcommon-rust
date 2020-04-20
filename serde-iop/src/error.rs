use std::error::Error as StdError;
use std::fmt;

#[derive(Debug, PartialEq)]
pub enum Error {
    Unimplemented(&'static str),
    MissingTag,
    UnknownLen,
    InputTooShort,
    InvalidEncoding,
    TrailingCharacters,
    Custom(String),
}
pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Unimplemented(name) => write!(fmt, "serialization of {} not implemented", name),
            Error::MissingTag => write!(fmt, "{}", self),
            Error::UnknownLen => write!(fmt, "{}", self),
            Error::InputTooShort => write!(fmt, "{}", self),
            Error::InvalidEncoding => write!(fmt, "{}", self),
            Error::TrailingCharacters => write!(fmt, "{}", self),
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
            Error::InputTooShort => "deserializing failed as input is too short",
            Error::InvalidEncoding => "binary encoding invalid",
            Error::TrailingCharacters => "trailing characters after unpacking",
            Error::Custom(msg) => msg,
        }
    }
}

impl serde::ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::Custom(msg.to_string())
    }
}

impl serde::de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::Custom(msg.to_string())
    }
}
