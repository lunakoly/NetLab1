use crate::{ErrorKind, Result};
use crate::communication::{ReadMessage, WriteMessage};
use crate::communication::bson::{BsonReader, BsonWriter};

use crate::capped_reader::{
    CappedReader,
    IntoCappedReader,
};

use std::io::Read;

use std::net::TcpStream;

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

impl<R: Read, M> XXsonReader<R, M> {
    pub fn new(capped_reader: CappedReader<R>) -> XXsonReader<R, M> {
        XXsonReader {
            backend: BsonReader::new(capped_reader),
            phantom: PhantomData,
        }
    }
}

// TODO: review

impl<'a, R: Read> ReadMessage<ClientMessage> for XXsonReader<R, ClientMessage> {
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

impl<'a, R: Read> ReadMessage<ServerMessage> for XXsonReader<R, ServerMessage> {
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

pub struct XXsonWriter<M> {
    backend: BsonWriter,
    phantom: PhantomData<M>,
}

impl<M> XXsonWriter<M> {
    pub fn new(stream: TcpStream) -> XXsonWriter<M> {
        XXsonWriter {
            backend: BsonWriter::new(stream),
            phantom: PhantomData,
        }
    }
}

impl WriteMessage<&ClientMessage> for XXsonWriter<ClientMessage> {
    fn write(&mut self, message: &ClientMessage) -> Result<()> {
        let serialized = bson::to_bson(message)?;

        match serialized.as_document() {
            Some(document) => self.backend.write(document),
            None => Err(ErrorKind::NothingToRead.into())
        }
    }
}

impl WriteMessage<&ServerMessage> for XXsonWriter<ServerMessage> {
    fn write(&mut self, message: &ServerMessage) -> Result<()> {
        let serialized = bson::to_bson(message)?;

        match serialized.as_document() {
            Some(document) => self.backend.write(document),
            None => Err(ErrorKind::NothingToRead.into())
        }
    }
}

pub struct ClientSideConnection {
    pub writer: XXsonWriter<ClientMessage>,
    pub reader: XXsonReader<TcpStream, ServerMessage>,
}

pub struct ServerSideConnection {
    pub writer: XXsonWriter<ServerMessage>,
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
    fn get_raw_tcp_stream(&mut self) -> &mut TcpStream;
}

impl Connection for ClientSideConnection {
    fn get_raw_tcp_stream(&mut self) -> &mut TcpStream {
        &mut self.writer.backend.stream
    }
}

impl Connection for ServerSideConnection {
    fn get_raw_tcp_stream(&mut self) -> &mut TcpStream {
        &mut self.writer.backend.stream
    }
}

pub trait Visualize {
    fn visualize(&self, connection: &mut dyn Connection) -> Result<()>;
}

impl Visualize for ClientMessage {
    fn visualize(&self, connection: &mut dyn Connection) -> Result<()> {
        match self {
            ClientMessage::Text { text } => {
                let address = connection.get_raw_tcp_stream().peer_addr()?.to_string();
                println!("[{}] {}", address, text);
            }
        };

        Ok(())
    }
}

impl Visualize for ServerMessage {
    fn visualize(&self, _: &mut dyn Connection) -> Result<()> {
        match self {
            ServerMessage::Text { text, name } => {
                println!("[{}] {}", name, text);
            }
        };

        Ok(())
    }
}
