pub mod connection;

use std::io::{Read, Write};
use std::marker::{PhantomData};
use std::fmt::{Display, Formatter};

use crate::{ErrorKind, Result};
use crate::communication::{ReadMessage, WriteMessage};
use crate::communication::bson::{BsonReader, BsonWriter};
use crate::capped_reader::{CappedReader};

use serde::{Serialize, Deserialize};

use bson::doc;

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    Text { text: String },
    Leave,
    Rename { new_name: String },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    Text { text: String, name: String, time: bson::DateTime },
    NewUser { name: String, time: bson::DateTime },
    Interrupt { name: String, time: bson::DateTime },
    UserLeaves { name: String, time: bson::DateTime },
    Support { text: String },
    UserRenamed { old_name: String, new_name: String },
}

pub struct XXsonReader<R, M> {
    backend: BsonReader<R>,
    phantom: PhantomData<M>,
}

impl<R, M> XXsonReader<R, M> {
    pub fn new(capped_reader: CappedReader<R>) -> XXsonReader<R, M> {
        XXsonReader {
            backend: BsonReader::new(capped_reader),
            phantom: PhantomData,
        }
    }
}

// TODO: review

impl<R: Read> ReadMessage<ClientMessage> for XXsonReader<R, ClientMessage> {
    fn read(&mut self) -> Result<ClientMessage> {
        match self.backend.read() {
            Ok(it) => {
                let message: ClientMessage = bson::from_bson(it.into())?;
                Ok(message)
            }
            Err(error) => {
                Err(error.into())
            }
        }
    }
}

impl<R: Read> ReadMessage<ServerMessage> for XXsonReader<R, ServerMessage> {
    fn read(&mut self) -> Result<ServerMessage> {
        match self.backend.read() {
            Ok(it) => {
                let message: ServerMessage = bson::from_bson(it.into())?;
                Ok(message)
            }
            Err(error) => {
                Err(error.into())
            }
        }
    }
}

pub struct XXsonWriter<W, M> {
    backend: BsonWriter<W>,
    phantom: PhantomData<M>,
}

impl<W, M> XXsonWriter<W, M> {
    pub fn new(stream: W) -> XXsonWriter<W, M> {
        XXsonWriter {
            backend: BsonWriter::new(stream),
            phantom: PhantomData,
        }
    }
}

impl<W: Write> WriteMessage<ClientMessage> for XXsonWriter<W, ClientMessage> {
    fn write(&mut self, message: &ClientMessage) -> Result<()> {
        let serialized = bson::to_bson(message)?;

        if let Some(it) = serialized.as_document() {
            self.backend.write(it)
        } else if let Some(it) = serialized.as_str() {
            let wrapper = doc! {
                it: {}
            };

            self.backend.write(&wrapper)
        } else {
            Err(ErrorKind::NothingToRead.into())
        }
    }
}

impl<W: Write> WriteMessage<ServerMessage> for XXsonWriter<W, ServerMessage> {
    fn write(&mut self, message: &ServerMessage) -> Result<()> {
        let serialized = bson::to_bson(message)?;

        match serialized.as_document() {
            Some(document) => self.backend.write(document),
            None => Err(ErrorKind::NothingToRead.into())
        }
    }
}

impl Display for ServerMessage {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self {
            ServerMessage::Text { text, name, time } => {
                let the_time = time.to_chrono();
                write!(formatter, "<{}> [{}] {}", the_time, name, text)
            }
            ServerMessage::NewUser { name, .. } => {
                write!(formatter, "~~ Meet our new mate: {} ~~", name)
            }
            ServerMessage::Interrupt { name, .. } => {
                write!(formatter, "~~ Press F, {} ~~", name)
            }
            ServerMessage::UserLeaves { name, .. } => {
                write!(formatter, "~~ {} leaves the party ~~", name)
            }
            ServerMessage::Support { text } => {
                write!(formatter, "(Server) {}", &text)
            }
            ServerMessage::UserRenamed { old_name, new_name } => {
                write!(formatter, "~~ He once used to be {}, but now he is {} ~~", &old_name, &new_name)
            }
        }
    }
}
