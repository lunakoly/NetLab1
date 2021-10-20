pub mod json;
pub mod bson;
pub mod xxson;

use crate::{Result, Error, ErrorKind};

pub const DEFAULT_PORT: u32 = 6969;

pub trait ReadMessage<M> {
    fn read_message(&mut self) -> Result<M>;
}

pub trait WriteMessage<M> {
    fn write_message(&mut self, message: &M) -> Result<()>;
}

pub trait ReadMessageWithContext<M, C, W> {
    fn read_message_with_context(
        &mut self,
        context: C,
        writer: W,
    ) -> Result<M>;
}

pub trait WriteMessageWithContext<M, C> {
    fn write_message_with_context(&mut self, message: &M, context: C) -> Result<()>;
}

pub fn explain_common_error(error: &Error) -> String {
    match &error.kind {
        ErrorKind::Io { source: io_error } => match io_error.kind() {
            // This particular error has a very
            // specific meaning in terms of communication
            std::io::ErrorKind::InvalidData => {
                "Too much data for a single message, disconnecting".to_owned()
            }
            _ => format!("{}", error)
        }
        _ => format!("{}", error)
    }
}

pub enum MessageProcessing {
    Proceed,
    Stop,
}
