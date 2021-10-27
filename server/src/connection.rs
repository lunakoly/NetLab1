use std::net::{TcpStream, SocketAddr};
use std::collections::{HashMap};
use std::fs::{File};

use shared::{Result};
use shared::shared::map::{SharedMap};
use shared::shared::{Shared, IntoShared};

use shared::communication::{
    ReadMessage,
    WriteMessage,
};

use shared::communication::arson::{ArsonReader, ArsonWriter};

use shared::connection::messages::{
    CommonMessage,
    ClientMessage,
    ServerMessage,
    MAXIMUM_MESSAGE_SIZE,
};

use shared::connection::{Context, Connection, WithConnection};
use shared::connection::sharers::{FileSharer, FileSharers};

pub type NamesMap = SharedMap<String, String>;
pub type Clients = SharedMap<String, Shared<ArsonServerSession>>;

pub struct ServerContext {
    common: Context,
    names: NamesMap,
    clients: Clients,
}

impl ServerContext {
    pub fn new(
        stream: Shared<TcpStream>,
        sharers: FileSharers,
        names: NamesMap,
        clients: Clients,
    ) -> ServerContext {
        ServerContext {
            common: Context::new(stream, sharers),
            names: names,
            clients: clients,
        }
    }
}

impl WithConnection for ServerContext {
    fn connection(&self) -> &dyn Connection {
        &self.common
    }

    fn connection_mut(&mut self) -> &mut dyn Connection {
        &mut self.common
    }
}

pub trait ServerConnection: Connection {
    fn name(&self) -> Result<String>;
    fn broadcast(&mut self, message: &ServerMessage) -> Result<()>;
    fn rename(&mut self, new_name: &str) -> Result<Option<ServerMessage>>;
    fn remove_from_clients(&mut self) -> Result<()>;
}

impl ServerConnection for ServerContext {
    fn name(&self) -> Result<String> {
        let address = self.remote_address()?.to_string();

        let proper = if let Some(it) = self.names.get_clone(&address)? {
            it
        } else {
            address
        };

        Ok(proper)
    }

    fn broadcast(&mut self, message: &ServerMessage) -> Result<()> {
        let mut the_clients = self.clients.write()?;

        for (_, connection) in the_clients.iter_mut() {
            connection.write_message(message)?;
        }

        Ok(())
    }

    fn rename(&mut self, new_name: &str) -> Result<Option<ServerMessage>> {
        if new_name.contains('.') || new_name.contains(':') {
            let message = ServerMessage::Support {
                text: "Your name can't contain '.'s or ':'s".to_owned()
            };

            return Ok(Some(message))
        }

        let cloned_names = self.names.clone();
        let mut the_names = cloned_names.write()?;

        if the_names.values().any(|it| it == &new_name) {
            let message = ServerMessage::Support {
                text: "This name has already been taken, choose another one".to_owned()
            };

            return Ok(Some(message))
        }

        let address = self.remote_address()?.to_string();

        let old_name = if let Some(it) = the_names.get(&address) {
            it.clone()
        } else {
            address.clone()
        };

        the_names.insert(address.clone(), new_name.to_owned());

        let response = ServerMessage::UserRenamed {
            old_name: old_name,
            new_name: new_name.to_owned(),
        };

        self.broadcast(&response)?;
        Ok(None)
    }

    fn remove_from_clients(&mut self) -> Result<()> {
        let address = self.remote_address()?.to_string();

        // In fact, the corresponding
        // writer must always be present.
        self.clients.remove(&address)?;
        self.names.remove(&address)?;

        Ok(())
    }
}

pub trait WithServerConnection: WithConnection {
    fn server_connection(&self) -> &dyn ServerConnection;
    fn server_connection_mut(&mut self) -> &mut dyn ServerConnection;
}

impl<W: WithServerConnection> ServerConnection for W {
    fn name(&self) -> Result<String> {
        self.server_connection().name()
    }

    fn broadcast(&mut self, message: &ServerMessage) -> Result<()> {
        self.server_connection_mut().broadcast(message)
    }

    fn rename(&mut self, new_name: &str) -> Result<Option<ServerMessage>> {
        self.server_connection_mut().rename(new_name)
    }

    fn remove_from_clients(&mut self) -> Result<()> {
        self.server_connection_mut().remove_from_clients()
    }
}

impl<T: ServerConnection> ServerConnection for Shared<T> {
    fn name(&self) -> Result<String> {
        self.inner.read()?.name()
    }

    fn broadcast(&mut self, message: &ServerMessage) -> Result<()> {
        self.inner.write()?.broadcast(message)
    }

    fn rename(&mut self, new_name: &str) -> Result<Option<ServerMessage>> {
        self.inner.write()?.rename(new_name)
    }

    fn remove_from_clients(&mut self) -> Result<()> {
        self.inner.write()?.remove_from_clients()
    }
}

#[derive(Clone)]
pub struct ArsonServerSession {
    context: Shared<ServerContext>,
    reader: Shared<ArsonReader<Shared<TcpStream>>>,
    writer: Shared<ArsonWriter<Shared<TcpStream>>>,
}

impl ArsonServerSession {
    pub fn new(
        context: Shared<ServerContext>,
        reader: Shared<ArsonReader<Shared<TcpStream>>>,
        writer: Shared<ArsonWriter<Shared<TcpStream>>>,
    ) -> ArsonServerSession {
        ArsonServerSession {
            context: context,
            reader: reader,
            writer: writer,
        }
    }
}

impl ReadMessage<ClientMessage> for ArsonServerSession {
    fn read_message(&mut self) -> Result<ClientMessage> {
        self.reader.read_message()
    }
}

impl WriteMessage<ServerMessage> for ArsonServerSession {
    fn write_message(&mut self, message: &ServerMessage) -> Result<()> {
        self.writer.write_message(message)
    }
}

impl WriteMessage<CommonMessage> for ArsonServerSession {
    fn write_message(&mut self, message: &CommonMessage) -> Result<()> {
        let wrapped = ServerMessage::Common {
            common: message.clone()
        };

        self.writer.write_message(&wrapped)
    }
}

impl Connection for ArsonServerSession {
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

impl ServerConnection for ArsonServerSession {
    fn name(&self) -> Result<String> {
        self.context.name()
    }

    fn broadcast(&mut self, message: &ServerMessage) -> Result<()> {
        self.context.broadcast(message)
    }

    fn rename(&mut self, new_name: &str) -> Result<Option<ServerMessage>> {
        self.context.rename(new_name)
    }

    fn remove_from_clients(&mut self) -> Result<()> {
        self.context.remove_from_clients()
    }
}

pub trait ServerSession: ServerConnection
    + ReadMessage<ClientMessage>
    + WriteMessage<ServerMessage>
    + WriteMessage<CommonMessage>
    + Clone + Send + Sync {}

impl ServerSession for ArsonServerSession {}

pub fn build_connection(
    stream: TcpStream,
    names: NamesMap,
    clients: Clients,
) -> Result<(ArsonServerSession, ArsonServerSession)> {
    let reading_stream = stream.try_clone()?.shared();
    let writing_stream = stream.shared();

    let reader = ArsonReader::new(reading_stream.clone(), MAXIMUM_MESSAGE_SIZE).shared();
    let writer = ArsonWriter::new(writing_stream.clone()).shared();
    let sharers = HashMap::new().shared();

    let reader_context = ArsonServerSession::new(
        ServerContext::new(reading_stream, sharers.clone(), names.clone(), clients.clone()).shared(),
        reader.clone(),
        writer.clone(),
    );

    let writer_context = ArsonServerSession::new(
        ServerContext::new(writing_stream, sharers.clone(), names, clients).shared(),
        reader.clone(),
        writer.clone(),
    );

    Ok((reader_context, writer_context))
}
