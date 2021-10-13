use std::net::{TcpStream, SocketAddr};

use crate::{Result};
use crate::shared::{Shared};
use crate::shared::map::{SharedMap};
use crate::communication::{ReadMessage, WriteMessage};

use super::{XXsonReader, XXsonWriter};

use super::{
    ClientMessage,
    ServerMessage,
};

pub struct Context {
    stream: Shared<TcpStream>,
}

impl Context {
    pub fn new(stream: Shared<TcpStream>) -> Context {
        Context {
            stream: stream,
        }
    }
}

pub trait Connection {
    fn remote_address(&self) -> Result<SocketAddr>;
}

impl Connection for Context {
    fn remote_address(&self) -> Result<SocketAddr> {
        Ok(self.stream.inner.read()?.peer_addr()?)
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
}

pub struct ClientContext {
    common: Context,
    reader: Shared<XXsonReader<Shared<TcpStream>, ServerMessage>>,
    writer: Shared<XXsonWriter<Shared<TcpStream>, ClientMessage>>,
}

impl ClientContext {
    pub fn new(
        stream: Shared<TcpStream>,
        reader: Shared<XXsonReader<Shared<TcpStream>, ServerMessage>>,
        writer: Shared<XXsonWriter<Shared<TcpStream>, ClientMessage>>,
    ) -> ClientContext {
        ClientContext {
            common: Context::new(stream),
            reader: reader,
            writer: writer,
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

impl ReadMessage<ServerMessage> for ClientContext {
    fn read_message(&mut self) -> Result<ServerMessage> {
        self.reader.read_message()
    }
}

impl WriteMessage<ClientMessage> for ClientContext {
    fn write_message(&mut self, message: &ClientMessage) -> Result<()> {
        self.writer.write_message(message)
    }
}

pub trait ClientConnection: Connection + ReadMessage<ServerMessage> + WriteMessage<ClientMessage> {}

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
}

impl ClientConnection for Shared<ClientContext> {}

pub type NamesMap = SharedMap<String, String>;
pub type Clients = SharedMap<String, Shared<ServerContext>>;

pub struct ServerContext {
    common: Context,
    names: NamesMap,
    clients: Clients,
    reader: Shared<XXsonReader<Shared<TcpStream>, ClientMessage>>,
    writer: Shared<XXsonWriter<Shared<TcpStream>, ServerMessage>>,
}

impl ServerContext {
    pub fn new(
        stream: Shared<TcpStream>,
        names: NamesMap,
        clients: Clients,
        reader: Shared<XXsonReader<Shared<TcpStream>, ClientMessage>>,
        writer: Shared<XXsonWriter<Shared<TcpStream>, ServerMessage>>,
    ) -> ServerContext {
        ServerContext {
            common: Context::new(stream),
            names: names,
            clients: clients,
            reader: reader,
            writer: writer,
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

impl ReadMessage<ClientMessage> for ServerContext {
    fn read_message(&mut self) -> Result<ClientMessage> {
        self.reader.read_message()
    }
}

impl WriteMessage<ServerMessage> for ServerContext {
    fn write_message(&mut self, message: &ServerMessage) -> Result<()> {
        self.writer.write_message(message)
    }
}

pub trait ServerConnection: Connection + ReadMessage<ClientMessage> + WriteMessage<ServerMessage> {
    fn name(&self) -> Result<String>;
    fn broadcast(&mut self, message: &ServerMessage) -> Result<()>;
    fn rename(&mut self, new_name: &str) -> Result<()>;
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

    fn rename(&mut self, new_name: &str) -> Result<()> {
        if new_name.contains('.') || new_name.contains(':') {
            let message = ServerMessage::Support {
                text: "Your name can't contain '.'s or ':'s".to_owned()
            };

            self.write_message(&message)?;
            return Ok(())
        }

        let cloned_names = self.names.clone();
        let mut the_names = cloned_names.write()?;

        if the_names.values().any(|it| it == &new_name) {
            let message = ServerMessage::Support {
                text: "This name has already been taken, choose another one".to_owned()
            };

            self.write_message(&message)?;
            return Ok(())
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
        Ok(())
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

pub trait WithServerConnection: WithConnection + ReadMessage<ClientMessage> + WriteMessage<ServerMessage> {
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

    fn rename(&mut self, new_name: &str) -> Result<()> {
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
}

impl ServerConnection for Shared<ServerContext> {
    fn name(&self) -> Result<String> {
        self.inner.read()?.name()
    }

    fn broadcast(&mut self, message: &ServerMessage) -> Result<()> {
        self.inner.write()?.broadcast(message)
    }

    fn rename(&mut self, new_name: &str) -> Result<()> {
        self.inner.write()?.rename(new_name)
    }

    fn remove_from_clients(&mut self) -> Result<()> {
        self.inner.write()?.remove_from_clients()
    }
}

pub fn build_client_connection(
    stream: TcpStream
) -> Result<(Shared<ClientContext>, Shared<ClientContext>)> {
    let reading_stream = Shared::new(stream.try_clone()?);
    let writing_stream = Shared::new(stream);

    let reader = Shared::new(
        XXsonReader::<_, ServerMessage>::new(reading_stream.clone())
    );

    let writer = Shared::new(
        XXsonWriter::<_, ClientMessage>::new(writing_stream.clone())
    );

    let reader_context = Shared::new(
        ClientContext::new(
            reading_stream, reader.clone(), writer.clone()
        )
    );

    let writer_context = Shared::new(
        ClientContext::new(
            writing_stream, reader, writer
        )
    );

    Ok((reader_context, writer_context))
}

pub fn build_server_connection(
    stream: TcpStream,
    names: NamesMap,
    clients: Clients,
) -> Result<(Shared<ServerContext>, Shared<ServerContext>)> {
    let reading_stream = Shared::new(stream.try_clone()?);
    let writing_stream = Shared::new(stream);

    let reader = Shared::new(
        XXsonReader::<_, ClientMessage>::new(reading_stream.clone())
    );

    let writer = Shared::new(
        XXsonWriter::<_, ServerMessage>::new(writing_stream.clone())
    );

    let reader_context = Shared::new(
        ServerContext::new(
            reading_stream, names.clone(), clients.clone(), reader.clone(), writer.clone()
        )
    );

    let writer_context = Shared::new(
        ServerContext::new(
            writing_stream, names, clients, reader, writer
        )
    );

    Ok((reader_context, writer_context))
}
