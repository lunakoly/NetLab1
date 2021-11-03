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

use shared::communication::arson::{ArsonScanner, ArsonWriter};

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
        reading_sharers: FileSharers,
        writing_sharers: Shared<Vec<FileSharer>>,
        names: NamesMap,
        clients: Clients,
    ) -> ServerContext {
        ServerContext {
            common: Context::new(
                stream,
                reading_sharers,
                writing_sharers
            ),
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

pub fn broadcast(clients: Clients, message: &ServerMessage) -> Result<()> {
    for (_, connection) in clients.write()?.iter_mut() {
        connection.write_message(message)?;
    }

    Ok(())
}

pub enum RenameResult {
    Success { old_name: String, new_name: String },
    Failure { reason: String },
}

pub trait ServerConnection: Connection {
    fn name(&self) -> Result<String>;
    fn names(&self) -> Result<NamesMap>;
    fn clients(&self) -> Result<Clients>;
    fn broadcast(&mut self, message: &ServerMessage) -> Result<()>;
    fn rename(&mut self, new_name: &str) -> Result<RenameResult>;
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

    fn names(&self) -> Result<NamesMap> {
        Ok(self.names.clone())
    }

    fn clients(&self) -> Result<Clients> {
        Ok(self.clients.clone())
    }

    fn broadcast(&mut self, message: &ServerMessage) -> Result<()> {
        broadcast(self.clients()?, message)
    }

    fn rename(&mut self, new_name: &str) -> Result<RenameResult> {
        if new_name.contains('.') || new_name.contains(':') {
            let message = RenameResult::Failure {
                reason: "Your name can't contain '.'s or ':'s".to_owned()
            };

            return Ok(message)
        }

        let cloned_names = self.names.clone();
        let mut the_names = cloned_names.write()?;

        if the_names.values().any(|it| it == &new_name) {
            let message = RenameResult::Failure {
                reason: "This name has already been taken, choose another one".to_owned()
            };

            return Ok(message)
        }

        let address = self.remote_address()?.to_string();

        let old_name = if let Some(it) = the_names.get(&address) {
            it.clone()
        } else {
            address.clone()
        };

        the_names.insert(address.clone(), new_name.to_owned());

        let response = RenameResult::Success {
            old_name: old_name,
            new_name: new_name.to_owned(),
        };

        Ok(response)
    }

    fn remove_from_clients(&mut self) -> Result<()> {
        let address = self.remote_address()?.to_string();

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

    fn names(&self) -> Result<NamesMap> {
        self.server_connection().names()
    }

    fn clients(&self) -> Result<Clients> {
        self.server_connection().clients()
    }

    fn broadcast(&mut self, message: &ServerMessage) -> Result<()> {
        self.server_connection_mut().broadcast(message)
    }

    fn rename(&mut self, new_name: &str) -> Result<RenameResult> {
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

    fn names(&self) -> Result<NamesMap> {
        self.inner.read()?.names()
    }

    fn clients(&self) -> Result<Clients> {
        self.inner.read()?.clients()
    }

    fn broadcast(&mut self, message: &ServerMessage) -> Result<()> {
        // Prevents the deadlock:
        // self.inner is no longer
        // locked after taking the
        // clients.
        broadcast(self.clients()?, message)
    }

    fn rename(&mut self, new_name: &str) -> Result<RenameResult> {
        self.inner.write()?.rename(new_name)
    }

    fn remove_from_clients(&mut self) -> Result<()> {
        // Prevents the deadlock
        let address = self.remote_address()?.to_string();
        self.clients()?.remove(&address)?;
        self.names()?.remove(&address)?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct ArsonServerSession {
    context: Shared<ServerContext>,
    reader: Shared<ArsonScanner<Shared<TcpStream>>>,
    writer: Shared<ArsonWriter<Shared<TcpStream>>>,
}

impl ArsonServerSession {
    pub fn new(
        context: Shared<ServerContext>,
        reader: Shared<ArsonScanner<Shared<TcpStream>>>,
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

    fn enqueu_sending_sharer(&mut self, sharer: FileSharer) -> Result<()> {
        self.context.enqueu_sending_sharer(sharer)
    }

    fn sending_sharers_queue(&self) -> Result<Shared<Vec<FileSharer>>> {
        self.context.sending_sharers_queue()
    }
}

impl ServerConnection for ArsonServerSession {
    fn name(&self) -> Result<String> {
        self.context.name()
    }

    fn names(&self) -> Result<NamesMap> {
        self.context.names()
    }

    fn clients(&self) -> Result<Clients> {
        self.context.clients()
    }

    fn broadcast(&mut self, message: &ServerMessage) -> Result<()> {
        self.context.broadcast(message)
    }

    fn rename(&mut self, new_name: &str) -> Result<RenameResult> {
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

impl<T: ServerSession> ServerSession for Shared<T> {}

pub fn build_connection(
    stream: TcpStream,
    names: NamesMap,
    clients: Clients,
) -> Result<(ArsonServerSession, ArsonServerSession)> {
    stream.set_nonblocking(true)?;

    let reading_stream = stream.try_clone()?.to_shared();
    let writing_stream = stream.to_shared();

    let reader = ArsonScanner::new(reading_stream.clone(), MAXIMUM_MESSAGE_SIZE).to_shared();
    let writer = ArsonWriter::new(writing_stream.clone()).to_shared();

    let reading_sharers = HashMap::new().to_shared();
    let writing_sharers = vec![].to_shared();

    let reader_context = ArsonServerSession::new(
        ServerContext::new(
            reading_stream,
            reading_sharers.clone(),
            writing_sharers.clone(),
            names.clone(),
            clients.clone()
        ).to_shared(),
        reader.clone(),
        writer.clone(),
    );

    let writer_context = ArsonServerSession::new(
        ServerContext::new(
            writing_stream,
            reading_sharers.clone(),
            writing_sharers.clone(),
            names,
            clients
        ).to_shared(),
        reader.clone(),
        writer.clone(),
    );

    Ok((reader_context, writer_context))
}
