pub mod json;
pub mod bson;
pub mod xxson;

use crate::{Result, Error, ErrorKind};

pub trait ReadMessage<M> {
    fn read(&mut self) -> Result<M>;
}

pub trait WriteMessage<M> {
    fn write(&mut self, message: M) -> Result<()>;
}

pub fn try_explain_common_error(error: &Error) -> Option<String> {
    match &error.kind {
        ErrorKind::Io { source: io_error } => match io_error.kind() {
            // This particular error has a very
            // specific meaning in terms of communication
            std::io::ErrorKind::InvalidData => {
                Some("Too much data for a single message, disconnecting".to_owned())
            }
            _ => None
        }
        _ => None
    }
}
