pub mod json;
pub mod bson;
pub mod xxson;

use crate::{Result, Error, ErrorKind};
use crate::shared_streams::{SharedStream};

pub const DEFAULT_PORT: u32 = 6969;

pub trait ReadMessage<M> {
    fn read_message(&mut self) -> Result<M>;
}

impl<M, R: ReadMessage<M>> ReadMessage<M> for SharedStream<R> {
    fn read_message(&mut self) -> Result<M> {
        self.stream.write()?.read_message()
    }
}

pub trait WriteMessage<M> {
    fn write_message(&mut self, message: &M) -> Result<()>;
}

impl<M, W: WriteMessage<M>> WriteMessage<M> for SharedStream<W> {
    fn write_message(&mut self, message: &M) -> Result<()> {
        self.stream.write()?.write_message(message)
    }
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
