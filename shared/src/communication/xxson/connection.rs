use std::net::{TcpStream, SocketAddr};
use std::collections::{HashMap};
use std::io::{Write};
use std::fs::{File};
use std::cmp::{min};

use crate::{Result};
use crate::shared::{Shared};
use crate::shared::map::{SharedMap};

use crate::communication::{
    ReadMessage,
    ReadMessageWithContext,
    WriteMessage,
    WriteMessageWithContext,
};

use super::{XXsonReader, XXsonWriter};

use super::messages::{
    CommonMessage,
    ClientMessage,
    ServerMessage,
};

use super::sharers::{FileSharer, FileSharers};

pub struct Context {
    stream: Shared<TcpStream>,
    sharers: FileSharers,
    nexd_id: usize,
}

impl Context {
    pub fn new(stream: Shared<TcpStream>, sharers: FileSharers) -> Context {
        Context {
            stream: stream,
            sharers: sharers,
            nexd_id: 0,
        }
    }
}

pub trait Connection {
    fn remote_address(&self) -> Result<SocketAddr>;

    fn free_id(&mut self) -> Result<usize>;

    fn prepare_sharer(
        &mut self,
        path: &str,
        file: File,
        name: &str,
    ) -> Result<()>;

    fn promote_sharer(
        &mut self,
        name: &str,
        size: usize,
        id: usize,
    ) -> Result<()>;

    fn accept_chunk(
        &mut self,
        data: &[u8],
        id: usize,
    ) -> Result<bool>;

    fn remove_unpromoted_sharer(&mut self, name: &str) -> Result<Option<FileSharer>>;

    fn remove_sharer(&mut self, id: usize) -> Result<Option<FileSharer>>;

    fn sharers_map(&mut self) -> Result<FileSharers>;
}

impl Connection for Context {
    fn remote_address(&self) -> Result<SocketAddr> {
        Ok(self.stream.inner.read()?.peer_addr()?)
    }

    fn free_id(&mut self) -> Result<usize> {
        let id = self.nexd_id;
        self.nexd_id += 1;
        Ok(id)
    }

    fn prepare_sharer(
        &mut self,
        path: &str,
        file: File,
        name: &str,
    ) -> Result<()> {
        let sharer = FileSharer {
            name: name.to_owned(),
            path: path.to_owned(),
            file: file,
            size: 0,
            written: 0,
        };

        self.sharers.insert(name.to_owned(), sharer)?;
        Ok(())
    }

    fn promote_sharer(
        &mut self,
        name: &str,
        size: usize,
        id: usize,
    ) -> Result<()> {
        let mut sharer = match self.sharers.remove(name)? {
            Some(it) => it,
            None => return Ok(())
        };

        sharer.size = size;

        let key = format!("{}", id);
        self.sharers.insert(key, sharer)?;

        Ok(())
    }

    fn accept_chunk(
        &mut self,
        data: &[u8],
        id: usize,
    ) -> Result<bool> {
        let mut sharers = self.sharers.write()?;
        let key = format!("{}", id);

        let sharer = if let Some(it) = sharers.get_mut(&key) {
            it
        } else {
            return Ok(false)
        };

        let writing_count = min(data.len(), sharer.rest());

        sharer.file.write(&data[..writing_count])?;
        sharer.written += writing_count;

        Ok(sharer.written >= sharer.size)
    }

    fn remove_unpromoted_sharer(&mut self, name: &str) -> Result<Option<FileSharer>> {
        self.sharers.remove(name)
    }

    fn remove_sharer(&mut self, id: usize) -> Result<Option<FileSharer>> {
        let key = format!("{}", id);
        self.sharers.remove(&key)
    }

    fn sharers_map(&mut self) -> Result<FileSharers> {
        Ok(self.sharers.clone())
    }
}

pub trait WithConnection {
    fn connection(&self) -> &dyn Connection;
    fn connection_mut(&mut self) -> &mut dyn Connection;
}

impl<W: WithConnection> Connection for W {
    fn remote_address(&self) -> Result<SocketAddr> {
        self.connection().remote_address()
    }

    fn free_id(&mut self) -> Result<usize> {
        self.connection_mut().free_id()
    }

    fn prepare_sharer(
        &mut self,
        path: &str,
        file: File,
        name: &str,
    ) -> Result<()> {
        self.connection_mut().prepare_sharer(path, file, name)
    }

    fn promote_sharer(
        &mut self,
        name: &str,
        size: usize,
        id: usize,
    ) -> Result<()> {
        self.connection_mut().promote_sharer(name, size, id)
    }

    fn accept_chunk(
        &mut self,
        data: &[u8],
        id: usize,
    ) -> Result<bool> {
        self.connection_mut().accept_chunk(data, id)
    }

    fn remove_unpromoted_sharer(&mut self, name: &str) -> Result<Option<FileSharer>> {
        self.connection_mut().remove_unpromoted_sharer(name)
    }

    fn remove_sharer(&mut self, id: usize) -> Result<Option<FileSharer>> {
        self.connection_mut().remove_sharer(id)
    }

    fn sharers_map(&mut self) -> Result<FileSharers> {
        self.connection_mut().sharers_map()
    }
}

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

impl Connection for Shared<ClientContext> {
    fn remote_address(&self) -> Result<SocketAddr> {
        self.inner.read()?.remote_address()
    }

    fn free_id(&mut self) -> Result<usize> {
        self.inner.write()?.free_id()
    }

    fn prepare_sharer(
        &mut self,
        path: &str,
        file: File,
        name: &str,
    ) -> Result<()> {
        self.inner.write()?.prepare_sharer(path, file, name)
    }

    fn promote_sharer(
        &mut self,
        name: &str,
        size: usize,
        id: usize,
    ) -> Result<()> {
        self.inner.write()?.promote_sharer(name, size, id)
    }

    fn accept_chunk(
        &mut self,
        data: &[u8],
        id: usize,
    ) -> Result<bool> {
        self.inner.write()?.accept_chunk(data, id)
    }

    fn remove_unpromoted_sharer(&mut self, name: &str) -> Result<Option<FileSharer>> {
        self.inner.write()?.remove_unpromoted_sharer(name)
    }

    fn remove_sharer(&mut self, id: usize) -> Result<Option<FileSharer>> {
        self.inner.write()?.remove_sharer(id)
    }

    fn sharers_map(&mut self) -> Result<FileSharers> {
        self.inner.write()?.sharers_map()
    }
}

impl ClientConnection for Shared<ClientContext> {}

pub type NamesMap = SharedMap<String, String>;
pub type Clients = SharedMap<String, Shared<ServerSessionData>>;

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

impl Connection for Shared<ServerContext> {
    fn remote_address(&self) -> Result<SocketAddr> {
        self.inner.read()?.remote_address()
    }

    fn free_id(&mut self) -> Result<usize> {
        self.inner.write()?.free_id()
    }

    fn prepare_sharer(
        &mut self,
        path: &str,
        file: File,
        name: &str,
    ) -> Result<()> {
        self.inner.write()?.prepare_sharer(path, file, name)
    }

    fn promote_sharer(
        &mut self,
        name: &str,
        size: usize,
        id: usize,
    ) -> Result<()> {
        self.inner.write()?.promote_sharer(name, size, id)
    }

    fn accept_chunk(
        &mut self,
        data: &[u8],
        id: usize,
    ) -> Result<bool> {
        self.inner.write()?.accept_chunk(data, id)
    }

    fn remove_unpromoted_sharer(&mut self, name: &str) -> Result<Option<FileSharer>> {
        self.inner.write()?.remove_unpromoted_sharer(name)
    }

    fn remove_sharer(&mut self, id: usize) -> Result<Option<FileSharer>> {
        self.inner.write()?.remove_sharer(id)
    }

    fn sharers_map(&mut self) -> Result<FileSharers> {
        self.inner.write()?.sharers_map()
    }
}

impl ServerConnection for Shared<ServerContext> {
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

pub struct ClientSessionData {
    context: Shared<ClientContext>,
    reader: Shared<XXsonReader<Shared<TcpStream>, Shared<ClientContext>>>,
    writer: Shared<XXsonWriter<Shared<TcpStream>, Shared<ClientContext>>>,
}

impl ClientSessionData {
    pub fn new(
        context: Shared<ClientContext>,
        reader: Shared<XXsonReader<Shared<TcpStream>, Shared<ClientContext>>>,
        writer: Shared<XXsonWriter<Shared<TcpStream>, Shared<ClientContext>>>,
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
        self.reader.read_message_with_context(self.context.clone(), self.writer.clone())
    }
}

impl WriteMessage<ClientMessage> for ClientSessionData {
    fn write_message(&mut self, message: &ClientMessage) -> Result<()> {
        self.writer.write_message_with_context(message, self.context.clone())
    }
}

impl WriteMessage<CommonMessage> for ClientSessionData {
    fn write_message(&mut self, message: &CommonMessage) -> Result<()> {
        self.writer.write_message_with_context(message, self.context.clone())
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

pub trait ClientSession: ClientConnection + ReadMessage<ServerMessage> + WriteMessage<ClientMessage> {}

impl ClientSession for ClientSessionData {}

pub struct ServerSessionData {
    context: Shared<ServerContext>,
    reader: Shared<XXsonReader<Shared<TcpStream>, Shared<ServerContext>>>,
    writer: Shared<XXsonWriter<Shared<TcpStream>, Shared<ServerContext>>>,
}

impl ServerSessionData {
    pub fn new(
        context: Shared<ServerContext>,
        reader: Shared<XXsonReader<Shared<TcpStream>, Shared<ServerContext>>>,
        writer: Shared<XXsonWriter<Shared<TcpStream>, Shared<ServerContext>>>,
    ) -> ServerSessionData {
        ServerSessionData {
            context: context,
            reader: reader,
            writer: writer,
        }
    }
}

impl ReadMessage<ClientMessage> for ServerSessionData {
    fn read_message(&mut self) -> Result<ClientMessage> {
        self.reader.read_message_with_context(self.context.clone(), self.writer.clone())
    }
}

impl WriteMessage<ServerMessage> for ServerSessionData {
    fn write_message(&mut self, message: &ServerMessage) -> Result<()> {
        self.writer.write_message_with_context(message, self.context.clone())
    }
}

impl WriteMessage<CommonMessage> for ServerSessionData {
    fn write_message(&mut self, message: &CommonMessage) -> Result<()> {
        self.writer.write_message_with_context(message, self.context.clone())
    }
}

impl Connection for ServerSessionData {
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

impl ServerConnection for ServerSessionData {
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

pub trait ServerSession: ServerConnection + ReadMessage<ClientMessage> + WriteMessage<ServerMessage> {}

impl ServerSession for ServerSessionData {}

pub fn build_client_connection(
    stream: TcpStream
) -> Result<(ClientSessionData, ClientSessionData)> {
    let reading_stream = Shared::new(stream.try_clone()?);
    let writing_stream = Shared::new(stream);

    let reader = Shared::new(
        XXsonReader::<_, Shared<ClientContext>>::new(reading_stream.clone())
    );

    let writer = Shared::new(
        XXsonWriter::<_, Shared<ClientContext>>::new(writing_stream.clone())
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

pub fn build_server_connection(
    stream: TcpStream,
    names: NamesMap,
    clients: Clients,
) -> Result<(ServerSessionData, ServerSessionData)> {
    let reading_stream = Shared::new(stream.try_clone()?);
    let writing_stream = Shared::new(stream);

    let reader = Shared::new(
        XXsonReader::<_, Shared<ServerContext>>::new(reading_stream.clone())
    );

    let writer = Shared::new(
        XXsonWriter::<_, Shared<ServerContext>>::new(writing_stream.clone())
    );

    let sharers = Shared::new(HashMap::new());

    let reader_context = ServerSessionData::new(
        Shared::new(
            ServerContext::new(reading_stream, sharers.clone(), names.clone(), clients.clone())
        ),
        reader.clone(),
        writer.clone(),
    );

    let writer_context = ServerSessionData::new(
        Shared::new(
            ServerContext::new(writing_stream, sharers.clone(), names, clients)
        ),
        reader.clone(),
        writer.clone(),
    );

    Ok((reader_context, writer_context))
}
