use std::collections::{HashMap};
use std::net::{TcpStream, SocketAddr};
use std::sync::{Arc, RwLock};
use std::cell::{RefCell};

use crate::{Result, ErrorKind};
use crate::ref_cell_stream::{RefCellStream};
use crate::capped_reader::{IntoCappedReader};
use crate::communication::{ReadMessage, WriteMessage};

use super::{XXsonReader, XXsonWriter};

use super::{
    ClientMessage,
    ServerMessage,
};

pub struct Context<'a> {
    stream: &'a RefCell<TcpStream>,
}

impl<'a> Context<'a> {
    pub fn new(stream: &RefCell<TcpStream>) -> Context {
        Context {
            stream: stream,
        }
    }
}

pub trait Connection {
    fn get_remote_address(&self) -> Result<SocketAddr>;
}

impl<'a> Connection for Context<'a> {
    fn get_remote_address(&self) -> Result<SocketAddr> {
        Ok(self.stream.borrow().peer_addr()?)
    }
}

pub trait WithConnection {
    fn get_connection(&self) -> &dyn Connection;
    fn get_connection_mut(&mut self) -> &mut dyn Connection;
}

impl<W: WithConnection> Connection for W {
    fn get_remote_address(&self) -> Result<SocketAddr> {
        self.get_connection().get_remote_address()
    }
}

pub struct ClientContext<'a> {
    common: Context<'a>,
}

impl<'a> ClientContext<'a> {
    pub fn new(stream: &RefCell<TcpStream>) -> ClientContext {
        ClientContext {
            common: Context::new(stream),
        }
    }
}

impl<'a> WithConnection for ClientContext<'a> {
    fn get_connection(&self) -> &dyn Connection {
        &self.common
    }

    fn get_connection_mut(&mut self) -> &mut dyn Connection {
        &mut self.common
    }
}

pub trait ClientConnection: Connection {}

impl<'a> ClientConnection for ClientContext<'a> {}

pub trait WithClientConnection: WithConnection {
    fn get_client_connection(&self) -> &dyn ClientConnection;
    fn get_client_connection_mut(&mut self) -> &mut dyn ClientConnection;
}

impl<W: WithClientConnection> ClientConnection for W {}

pub type NamesMap = Arc<RwLock<HashMap<String, String>>>;
pub type Clients<'a> = Arc<RwLock<HashMap<String, XXsonWriter<TcpStream, ServerMessage>>>>;

pub struct ServerContext<'a> {
    common: Context<'a>,
    names: NamesMap,
    clients: Clients<'a>,
}

impl<'a> ServerContext<'a> {
    pub fn new<'b>(
        stream: &'b RefCell<TcpStream>,
        names: NamesMap,
        clients: Clients<'b>,
    ) -> ServerContext<'b> {
        ServerContext {
            common: Context::new(stream),
            names: names,
            clients: clients,
        }
    }
}

impl<'a> WithConnection for ServerContext<'a> {
    fn get_connection(&self) -> &dyn Connection {
        &self.common
    }

    fn get_connection_mut(&mut self) -> &mut dyn Connection {
        &mut self.common
    }
}

pub trait ServerConnection: Connection {
    fn get_name(&self) -> Result<String>;
    fn broadcast(&mut self, message: &ServerMessage) -> Result<()>;
    fn rename(&mut self, new_name: &str) -> Result<()>;
    fn remove_current_writer(&mut self) -> Result<()>;
}

impl<'a> ServerConnection for ServerContext<'a> {
    fn get_name(&self) -> Result<String> {
        let address = self.get_remote_address()?.to_string();
        let it = self.names.read()?;

        let proper = if it.contains_key(&address) {
            it[&address].clone()
        } else {
            address
        };

        Ok(proper)
    }

    fn broadcast(&mut self, message: &ServerMessage) -> Result<()> {
        let mut the_clients = self.clients.write()?;

        for (_, writer) in the_clients.iter_mut() {
            writer.write(message)?;
        }

        Ok(())
    }

    fn rename(&mut self, new_name: &str) -> Result<()> {
        let cloned_names = self.names.clone();
        let mut the_names = cloned_names.write()?;
        let address = self.get_remote_address()?.to_string();
        let name_is_free = the_names.values().all(|it| it != &new_name);

        if name_is_free {
            let old_name = if the_names.contains_key(&address) {
                the_names[&address].clone()
            } else {
                address.clone()
            };

            the_names.insert(address.clone(), new_name.to_owned());

            let response = ServerMessage::UserRenamed {
                old_name: old_name,
                new_name: new_name.to_owned(),
            };

            self.broadcast(&response)?;
            return Ok(())
        }

        let mut the_clients = self.clients.write()?;

        let maybe_writer = if the_clients.contains_key(&address) {
            the_clients.get_mut(&address)
        } else {
            // The user was removed early.
            return Ok(())
        };

        let writer = if let Some(it) = maybe_writer {
            it
        } else {
            let kind = ErrorKind::NoWritingStreamFound {
                address: address
            };
            return Err(kind.into())
        };

        let message = ServerMessage::Support {
            text: "This name has already been taken, choose another one".to_owned()
        };

        writer.write(&message)?;
        Ok(())
    }

    fn remove_current_writer(&mut self) -> Result<()> {
        let address = self.get_remote_address()?.to_string();
        let mut the_clients = self.clients.write()?;

        // In fact, the corresponding
        // writer must always be present.
        if the_clients.contains_key(&address) {
            the_clients.remove(&address);
        }

        Ok(())
    }
}

pub trait WithServerConnection: WithConnection {
    fn get_server_connection(&self) -> &dyn ServerConnection;
    fn get_server_connection_mut(&mut self) -> &mut dyn ServerConnection;
}

impl<W: WithServerConnection> ServerConnection for W {
    fn get_name(&self) -> Result<String> {
        self.get_server_connection().get_name()
    }

    fn broadcast(&mut self, message: &ServerMessage) -> Result<()> {
        self.get_server_connection_mut().broadcast(message)
    }

    fn rename(&mut self, new_name: &str) -> Result<()> {
        self.get_server_connection_mut().rename(new_name)
    }

    fn remove_current_writer(&mut self) -> Result<()> {
        self.get_server_connection_mut().remove_current_writer()
    }
}

pub struct ClientReadingContext<'a> {
    connection: ClientContext<'a>,
    reader: XXsonReader<RefCellStream<'a, TcpStream>, ServerMessage>,
}

impl<'a> ClientReadingContext<'a> {
    pub fn new(stream: &RefCell<TcpStream>) -> ClientReadingContext {
        ClientReadingContext {
            connection: ClientContext::new(stream),
            reader: XXsonReader::new(RefCellStream::new(stream).capped()),
        }
    }
}

impl<'a> WithConnection for ClientReadingContext<'a> {
    fn get_connection(&self) -> &dyn Connection {
        &self.connection
    }

    fn get_connection_mut(&mut self) -> &mut dyn Connection {
        &mut self.connection
    }
}

impl<'a> WithClientConnection for ClientReadingContext<'a> {
    fn get_client_connection(&self) -> &dyn ClientConnection {
        &self.connection
    }

    fn get_client_connection_mut(&mut self) -> &mut dyn ClientConnection {
        &mut self.connection
    }
}

impl<'a> ReadMessage<ServerMessage> for ClientReadingContext<'a> {
    fn read(&mut self) -> Result<ServerMessage> {
        self.reader.read()
    }
}

pub trait ClientReadingConnection: ClientConnection + ReadMessage<ServerMessage> {}

impl<'a> ClientReadingConnection for ClientReadingContext<'a> {}

pub struct ClientWritingContext<'a> {
    connection: ClientContext<'a>,
    writer: XXsonWriter<RefCellStream<'a, TcpStream>, ClientMessage>,
}

impl<'a> ClientWritingContext<'a> {
    pub fn new(stream: &RefCell<TcpStream>) -> ClientWritingContext {
        ClientWritingContext {
            connection: ClientContext::new(stream),
            writer: XXsonWriter::new(RefCellStream::new(stream)),
        }
    }
}

impl<'a> WithConnection for ClientWritingContext<'a> {
    fn get_connection(&self) -> &dyn Connection {
        &self.connection
    }

    fn get_connection_mut(&mut self) -> &mut dyn Connection {
        &mut self.connection
    }
}

impl<'a> WithClientConnection for ClientWritingContext<'a> {
    fn get_client_connection(&self) -> &dyn ClientConnection {
        &self.connection
    }

    fn get_client_connection_mut(&mut self) -> &mut dyn ClientConnection {
        &mut self.connection
    }
}

impl<'a> WriteMessage<ClientMessage> for ClientWritingContext<'a> {
    fn write(&mut self, message: &ClientMessage) -> Result<()> {
        self.writer.write(message)
    }
}

pub trait ClientWritingConnection: ClientConnection + WriteMessage<ClientMessage> {}

impl<'a> ClientWritingConnection for ClientWritingContext<'a> {}

pub struct ServerReadingContext<'a> {
    connection: ServerContext<'a>,
    reader: XXsonReader<RefCellStream<'a, TcpStream>, ClientMessage>,
}

impl<'a> ServerReadingContext<'a> {
    pub fn new<'b>(
        stream: &'b RefCell<TcpStream>,
        names: NamesMap,
        clients: Clients<'b>,
    ) -> ServerReadingContext<'b> {
        ServerReadingContext {
            connection: ServerContext::new(stream, names, clients),
            reader: XXsonReader::new(RefCellStream::new(stream).capped()),
        }
    }
}

impl<'a> WithConnection for ServerReadingContext<'a> {
    fn get_connection(&self) -> &dyn Connection {
        &self.connection
    }

    fn get_connection_mut(&mut self) -> &mut dyn Connection {
        &mut self.connection
    }
}

impl<'a> WithServerConnection for ServerReadingContext<'a> {
    fn get_server_connection(&self) -> &dyn ServerConnection {
        &self.connection
    }

    fn get_server_connection_mut(&mut self) -> &mut dyn ServerConnection {
        &mut self.connection
    }
}

impl<'a> ReadMessage<ClientMessage> for ServerReadingContext<'a> {
    fn read(&mut self) -> Result<ClientMessage> {
        self.reader.read()
    }
}

pub trait ServerReadingConnection: ServerConnection + ReadMessage<ClientMessage> {}

impl<'a> ServerReadingConnection for ServerReadingContext<'a> {}

pub struct ServerWritingContext<'a> {
    connection: ServerContext<'a>,
    writer: XXsonWriter<RefCellStream<'a, TcpStream>, ServerMessage>,
}

impl<'a> ServerWritingContext<'a> {
    pub fn new<'b>(
        stream: &'b RefCell<TcpStream>,
        names: NamesMap,
        clients: Clients<'b>,
    ) -> ServerWritingContext<'b> {
        ServerWritingContext {
            connection: ServerContext::new(stream, names, clients),
            writer: XXsonWriter::new(RefCellStream::new(stream)),
        }
    }
}

impl<'a> WithConnection for ServerWritingContext<'a> {
    fn get_connection(&self) -> &dyn Connection {
        &self.connection
    }

    fn get_connection_mut(&mut self) -> &mut dyn Connection {
        &mut self.connection
    }
}

impl<'a> WithServerConnection for ServerWritingContext<'a> {
    fn get_server_connection(&self) -> &dyn ServerConnection {
        &self.connection
    }

    fn get_server_connection_mut(&mut self) -> &mut dyn ServerConnection {
        &mut self.connection
    }
}

impl<'a> WriteMessage<ServerMessage> for ServerWritingContext<'a> {
    fn write(&mut self, message: &ServerMessage) -> Result<()> {
        self.writer.write(message)
    }
}

pub trait ServerWritingConnection<'b>: ServerConnection + WriteMessage<ServerMessage> {
    fn to_writer(self) -> XXsonWriter<RefCellStream<'b, TcpStream>, ServerMessage>;
}

impl<'a> ServerWritingConnection<'a> for ServerWritingContext<'a> {
    fn to_writer(self) -> XXsonWriter<RefCellStream<'a, TcpStream>, ServerMessage> {
        self.writer
    }
}
