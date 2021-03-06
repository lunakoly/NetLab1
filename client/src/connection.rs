use std::net::{TcpStream, SocketAddr};
use std::collections::{HashMap};
use std::fs::{File};

use shared::{Result};
use shared::shared::{Shared, IntoShared};

use shared::communication::{
    ReadMessage,
    WriteMessage,
};

use shared::communication::arson::{ArsonScanner, ArsonWriter};

use shared::connection::messages::{
    CommonMessage,
    ClientMessage,
    ServerMessage,
    MAXIMUM_MESSAGE_SIZE,
};

use shared::connection::{Context, Connection, WithConnection};
use shared::connection::sharers::{FileSharer, FileSharers};

pub struct ClientContext {
    common: Context,
}

impl ClientContext {
    pub fn new(
        stream: Shared<TcpStream>,
        reading_sharers: FileSharers,
        writing_sharers: Shared<Vec<FileSharer>>,
    ) -> ClientContext {
        ClientContext {
            common: Context::new(
                stream,
                reading_sharers,
                writing_sharers
            ),
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
pub struct ArsonClientSession {
    context: Shared<ClientContext>,
    reader: Shared<ArsonScanner<Shared<TcpStream>>>,
    writer: Shared<ArsonWriter<Shared<TcpStream>>>,
}

impl ArsonClientSession {
    pub fn new(
        context: Shared<ClientContext>,
        reader: Shared<ArsonScanner<Shared<TcpStream>>>,
        writer: Shared<ArsonWriter<Shared<TcpStream>>>,
    ) -> ArsonClientSession {
        ArsonClientSession {
            context: context,
            reader: reader,
            writer: writer,
        }
    }
}

impl ReadMessage<ServerMessage> for ArsonClientSession {
    fn read_message(&mut self) -> Result<ServerMessage> {
        self.reader.read_message()
    }
}

impl WriteMessage<ClientMessage> for ArsonClientSession {
    fn write_message(&mut self, message: &ClientMessage) -> Result<()> {
        self.writer.write_message(message)
    }
}

impl WriteMessage<CommonMessage> for ArsonClientSession {
    fn write_message(&mut self, message: &CommonMessage) -> Result<()> {
        let wrapped = ClientMessage::Common {
            common: message.clone()
        };

        self.writer.write_message(&wrapped)
    }
}

impl Connection for ArsonClientSession {
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

    fn enqueu_sending_sharer(&mut self, sharer: FileSharer) -> Result<()> {
        self.context.enqueu_sending_sharer(sharer)
    }

    fn sending_sharers_queue(&self) -> Result<Shared<Vec<FileSharer>>> {
        self.context.sending_sharers_queue()
    }
}

impl ClientConnection for ArsonClientSession {}

pub trait ClientSession: ClientConnection
    + ReadMessage<ServerMessage>
    + WriteMessage<ClientMessage>
    + WriteMessage<CommonMessage>
    + Clone + Send + Sync {}

impl ClientSession for ArsonClientSession {}

impl<T: ClientSession> ClientSession for Shared<T> {}

pub fn build_connection(
    stream: TcpStream
) -> Result<(ArsonClientSession, ArsonClientSession)> {
    stream.set_nonblocking(true)?;
    // stream.set_nodelay(true)?;

    let reading_stream = stream.try_clone()?.to_shared();
    let writing_stream = stream.to_shared();

    let reader = ArsonScanner::new(reading_stream.clone(), MAXIMUM_MESSAGE_SIZE).to_shared();
    let writer = ArsonWriter::new(writing_stream.clone()).to_shared();

    let reading_sharers = HashMap::new().to_shared();
    let writing_sharers = vec![].to_shared();

    let reader_context = ArsonClientSession::new(
        ClientContext::new(
            reading_stream,
            reading_sharers.clone(),
            writing_sharers.clone(),
        ).to_shared(),
        reader.clone(),
        writer.clone(),
    );

    let writer_context = ArsonClientSession::new(
        ClientContext::new(
            writing_stream,
            reading_sharers.clone(),
            writing_sharers.clone(),
        ).to_shared(),
        reader.clone(),
        writer.clone(),
    );

    Ok((reader_context, writer_context))
}
