use crate::{ErrorKind, Result};
use crate::helpers::{NamesMap, get_name};
use crate::communication::{ReadMessage, WriteMessage};
use crate::communication::bson::{BsonReader, BsonWriter};

use crate::capped_reader::{
    CappedReader,
    IntoCappedReader,
};

use std::io::{Read, Write};

use std::net::{TcpStream, SocketAddr};

use serde::{Serialize, Deserialize};

use std::marker::PhantomData;

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    Text { text: String },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    Text { text: String, name: String },
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

impl<W: Write> WriteMessage<&ClientMessage> for XXsonWriter<W, ClientMessage> {
    fn write(&mut self, message: &ClientMessage) -> Result<()> {
        let serialized = bson::to_bson(message)?;

        match serialized.as_document() {
            Some(document) => self.backend.write(document),
            None => Err(ErrorKind::NothingToRead.into())
        }
    }
}

impl<W: Write> WriteMessage<&ServerMessage> for XXsonWriter<W, ServerMessage> {
    fn write(&mut self, message: &ServerMessage) -> Result<()> {
        let serialized = bson::to_bson(message)?;

        match serialized.as_document() {
            Some(document) => self.backend.write(document),
            None => Err(ErrorKind::NothingToRead.into())
        }
    }
}

pub struct ClientSideConnection {
    pub writer: XXsonWriter<TcpStream, ClientMessage>,
    pub reader: XXsonReader<TcpStream, ServerMessage>,
}

pub struct ServerSideConnection {
    pub writer: XXsonWriter<TcpStream, ServerMessage>,
    pub reader: XXsonReader<TcpStream, ClientMessage>,
}

impl ClientSideConnection {
    pub fn new(stream: TcpStream) -> Result<ClientSideConnection> {
        let connection = ClientSideConnection {
            writer: XXsonWriter::new(stream.try_clone()?),
            reader: XXsonReader::new(stream.capped()),
        };

        Ok(connection)
    }
}

impl ServerSideConnection {
    pub fn new(stream: TcpStream) -> Result<ServerSideConnection> {
        let connection = ServerSideConnection {
            writer: XXsonWriter::new(stream.try_clone()?),
            reader: XXsonReader::new(stream.capped()),
        };

        Ok(connection)
    }
}

pub trait Connection {
    fn get_remote_address(&self) -> Result<SocketAddr>;

    fn get_name(&self, names: NamesMap) -> Result<String> {
        let address = self.get_remote_address()?.to_string();
        get_name(names, address)
    }

    fn get_personality_prefix(&self, names: NamesMap) -> Result<String> {
        let name = self.get_name(names)?;
        let time = chrono::Utc::now();
        Ok(format!("<{}> [{}] ", time, name))
    }
}

impl<M> Connection for XXsonWriter<TcpStream, M> {
    fn get_remote_address(&self) -> Result<SocketAddr> {
        Ok(self.backend.stream.peer_addr()?)
    }
}

impl<M> Connection for XXsonReader<TcpStream, M> {
    fn get_remote_address(&self) -> Result<SocketAddr> {
        Ok(self.backend.stream.stream.peer_addr()?)
    }
}

impl Connection for ClientSideConnection {
    fn get_remote_address(&self) -> Result<SocketAddr> {
        self.writer.get_remote_address()
    }
}

impl Connection for ServerSideConnection {
    fn get_remote_address(&self) -> Result<SocketAddr> {
        self.writer.get_remote_address()
    }
}

pub trait VisualizeClientMessage {
    fn visualize(&self, names: NamesMap, connection: &dyn Connection) -> Result<()>;
}

impl VisualizeClientMessage for ClientMessage {
    fn visualize(&self, names: NamesMap, connection: &dyn Connection) -> Result<()> {
        match self {
            ClientMessage::Text { text } => {
                let prefix = connection.get_personality_prefix(names)?;
                println!("{}{}", prefix, text);
            }
        };

        Ok(())
    }
}

pub trait VisualizeServerMessage {
    fn visualize(&self, connection: &dyn Connection) -> Result<()>;
}

impl VisualizeServerMessage for ServerMessage {
    fn visualize(&self, _: &dyn Connection) -> Result<()> {
        match self {
            ServerMessage::Text { text, name } => {
                println!("[{}] {}", name, text);
            }
        };

        Ok(())
    }
}
