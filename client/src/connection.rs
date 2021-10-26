use std::net::{TcpStream, SocketAddr};
use std::collections::{HashMap};
use std::fs::{File};

use shared::{Result};
use shared::shared::{Shared};

use shared::communication::{
    ReadMessage,
    WriteMessage,
};

use shared::communication::arson::{ArsonReader, ArsonWriter};

use shared::connection::messages::{
    CommonMessage,
    ClientMessage,
    ServerMessage,
};

use shared::connection::{Context, Connection, WithConnection};
use shared::connection::sharers::{FileSharer, FileSharers};

pub struct ClientContext {
    common: Context,
}

impl ClientContext {
    pub fn new(stream: Shared<TcpStream>, sharers: FileSharers) -> ClientContext {
        ClientContext {
            common: Context::new(stream, sharers),
        }
    }
}

impl WithConnection for ClientContext {
    fn connection(&self) -> &dyn Connection {
        &self.common
    }

    fn connection_mut(&mut self) -> &mut dyn Connection {
        &mut self.common
    }
}

pub trait ClientConnection: Connection {}

impl<'a> ClientConnection for ClientContext {}

pub trait WithClientConnection: WithConnection + ReadMessage<ServerMessage> + WriteMessage<ClientMessage> {
    fn client_connection(&self) -> &dyn ClientConnection;
    fn client_connection_mut(&mut self) -> &mut dyn ClientConnection;
}

impl<W: WithClientConnection> ClientConnection for W {}

impl<T: ClientConnection> ClientConnection for Shared<T> {}

#[derive(Clone)]
pub struct ClientSessionData {
    context: Shared<ClientContext>,
    reader: Shared<ArsonReader<Shared<TcpStream>>>,
    writer: Shared<ArsonWriter<Shared<TcpStream>>>,
}

impl ClientSessionData {
    pub fn new(
        context: Shared<ClientContext>,
        reader: Shared<ArsonReader<Shared<TcpStream>>>,
        writer: Shared<ArsonWriter<Shared<TcpStream>>>,
    ) -> ClientSessionData {
        ClientSessionData {
            context: context,
            reader: reader,
            writer: writer,
        }
    }
}

impl ReadMessage<ServerMessage> for ClientSessionData {
    fn read_message(&mut self) -> Result<ServerMessage> {
        self.reader.read_message()
    }
}

impl WriteMessage<ClientMessage> for ClientSessionData {
    fn write_message(&mut self, message: &ClientMessage) -> Result<()> {
        self.writer.write_message(message)
    }
}

impl WriteMessage<CommonMessage> for ClientSessionData {
    fn write_message(&mut self, message: &CommonMessage) -> Result<()> {
        let wrapped = ClientMessage::Common {
            common: message.clone()
        };

        self.writer.write_message(&wrapped)
    }
}

impl Connection for ClientSessionData {
    fn remote_address(&self) -> Result<SocketAddr> {
        self.context.remote_address()
    }

    fn free_id(&mut self) -> Result<usize> {
        self.context.free_id()
    }

    fn prepare_sharer(
        &mut self,
        path: &str,
        file: File,
        name: &str,
    ) -> Result<()> {
        self.context.prepare_sharer(path, file, name)
    }

    fn promote_sharer(
        &mut self,
        name: &str,
        size: usize,
        id: usize,
    ) -> Result<()> {
        self.context.promote_sharer(name, size, id)
    }

    fn accept_chunk(
        &mut self,
        data: &[u8],
        id: usize,
    ) -> Result<bool> {
        self.context.accept_chunk(data, id)
    }

    fn remove_unpromoted_sharer(&mut self, name: &str) -> Result<Option<FileSharer>> {
        self.context.remove_unpromoted_sharer(name)
    }

    fn remove_sharer(&mut self, id: usize) -> Result<Option<FileSharer>> {
        self.context.remove_sharer(id)
    }

    fn sharers_map(&mut self) -> Result<FileSharers> {
        self.context.sharers_map()
    }
}

impl ClientConnection for ClientSessionData {}

pub trait ClientSession: ClientConnection
    + ReadMessage<ServerMessage>
    + WriteMessage<ClientMessage>
    + WriteMessage<CommonMessage>
    + Clone + Send + Sync {}

impl ClientSession for ClientSessionData {}

pub fn build_connection(
    stream: TcpStream
) -> Result<(ClientSessionData, ClientSessionData)> {
    let reading_stream = Shared::new(stream.try_clone()?);
    let writing_stream = Shared::new(stream);

    let reader = Shared::new(
        ArsonReader::new(reading_stream.clone())
    );

    let writer = Shared::new(
        ArsonWriter::new(writing_stream.clone())
    );

    let sharers = Shared::new(HashMap::new());

    let reader_context = ClientSessionData::new(
        Shared::new(
            ClientContext::new(reading_stream, sharers.clone())
        ),
        reader.clone(),
        writer.clone(),
    );

    let writer_context = ClientSessionData::new(
        Shared::new(
            ClientContext::new(writing_stream, sharers.clone())
        ),
        reader.clone(),
        writer.clone(),
    );

    Ok((reader_context, writer_context))
}
