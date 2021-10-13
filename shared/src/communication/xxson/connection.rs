use std::net::{TcpStream, SocketAddr};

use crate::{Result};
use crate::safe_map::{SafeMap, SimpleMap};
use crate::shared_streams::{SharedStream};
use crate::communication::{ReadMessage, WriteMessage};

use super::{XXsonReader, XXsonWriter};

use super::{
    ClientMessage,
    ServerMessage,
};

pub struct Context {
    stream: SharedStream<TcpStream>,
}

impl Context {
    pub fn new(stream: SharedStream<TcpStream>) -> Context {
        Context {
            stream: stream,
        }
    }
}

pub trait Connection {
    fn get_remote_address(&self) -> Result<SocketAddr>;
}

impl Connection for Context {
    fn get_remote_address(&self) -> Result<SocketAddr> {
        Ok(self.stream.stream.read()?.peer_addr()?)
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

pub struct ClientContext {
    common: Context,
    reader: SharedStream<XXsonReader<SharedStream<TcpStream>, ServerMessage>>,
    writer: SharedStream<XXsonWriter<SharedStream<TcpStream>, ClientMessage>>,
}

impl ClientContext {
    pub fn new(
        stream: SharedStream<TcpStream>,
        reader: SharedStream<XXsonReader<SharedStream<TcpStream>, ServerMessage>>,
        writer: SharedStream<XXsonWriter<SharedStream<TcpStream>, ClientMessage>>,
    ) -> ClientContext {
        ClientContext {
            common: Context::new(stream),
            reader: reader,
            writer: writer,
        }
    }
}

impl WithConnection for ClientContext {
    fn get_connection(&self) -> &dyn Connection {
        &self.common
    }

    fn get_connection_mut(&mut self) -> &mut dyn Connection {
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
    fn get_client_connection(&self) -> &dyn ClientConnection;
    fn get_client_connection_mut(&mut self) -> &mut dyn ClientConnection;
}

impl<W: WithClientConnection> ClientConnection for W {}

pub type NamesMap = SafeMap<String, String>;
pub type Clients = SafeMap<String, ServerContext>;

pub struct ServerContext {
    common: Context,
    names: NamesMap,
    clients: Clients,
    reader: SharedStream<XXsonReader<SharedStream<TcpStream>, ClientMessage>>,
    writer: SharedStream<XXsonWriter<SharedStream<TcpStream>, ServerMessage>>,
}

impl ServerContext {
    pub fn new(
        stream: SharedStream<TcpStream>,
        names: NamesMap,
        clients: Clients,
        reader: SharedStream<XXsonReader<SharedStream<TcpStream>, ClientMessage>>,
        writer: SharedStream<XXsonWriter<SharedStream<TcpStream>, ServerMessage>>,
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
    fn get_connection(&self) -> &dyn Connection {
        &self.common
    }

    fn get_connection_mut(&mut self) -> &mut dyn Connection {
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
    fn get_name(&self) -> Result<String>;
    fn broadcast(&mut self, message: &ServerMessage) -> Result<()>;
    fn write_to_current(&mut self, message: &ServerMessage) -> Result<()>;
    fn rename(&mut self, new_name: &str) -> Result<()>;
    fn remove_current_writer(&mut self) -> Result<()>;
}

impl ServerConnection for ServerContext {
    fn get_name(&self) -> Result<String> {
        let address = self.get_remote_address()?.to_string();

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
            connection.writer.write_message(message)?;
        }

        Ok(())
    }

    fn write_to_current(&mut self, message: &ServerMessage) -> Result<()> {
        let address = self.get_remote_address()?.to_string();
        let mut the_clients = self.clients.write()?;

        if let Some(it) = the_clients.get_mut(&address) {
            it.writer.write_message(message)
        } else {
            // The user was removed early.
            Ok(())
        }
    }

    fn rename(&mut self, new_name: &str) -> Result<()> {
        if new_name.contains('.') || new_name.contains(':') {
            let message = ServerMessage::Support {
                text: "Your name can't contain '.'s or ':'s".to_owned()
            };

            self.write_to_current(&message)?;
            return Ok(())
        }

        let cloned_names = self.names.clone();
        let mut the_names = cloned_names.write()?;

        if the_names.values().any(|it| it == &new_name) {
            let message = ServerMessage::Support {
                text: "This name has already been taken, choose another one".to_owned()
            };

            self.write_to_current(&message)?;
            return Ok(())
        }

        let address = self.get_remote_address()?.to_string();

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

    fn remove_current_writer(&mut self) -> Result<()> {
        let address = self.get_remote_address()?.to_string();

        // In fact, the corresponding
        // writer must always be present.
        self.clients.remove(&address)?;
        self.names.remove(&address)?;

        Ok(())
    }
}

pub trait WithServerConnection: WithConnection + ReadMessage<ClientMessage> + WriteMessage<ServerMessage> {
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

    fn write_to_current(&mut self, message: &ServerMessage) -> Result<()> {
        self.get_server_connection_mut().write_to_current(message)
    }

    fn rename(&mut self, new_name: &str) -> Result<()> {
        self.get_server_connection_mut().rename(new_name)
    }

    fn remove_current_writer(&mut self) -> Result<()> {
        self.get_server_connection_mut().remove_current_writer()
    }
}

pub fn build_client_connection(
    stream: TcpStream
) -> Result<(ClientContext, ClientContext)> {
    let reading_stream = SharedStream::new(stream.try_clone()?);
    let writing_stream = SharedStream::new(stream);

    let reader = SharedStream::new(
        XXsonReader::<_, ServerMessage>::new(reading_stream.clone())
    );

    let writer = SharedStream::new(
        XXsonWriter::<_, ClientMessage>::new(writing_stream.clone())
    );

    let reader_context = ClientContext::new(
        reading_stream, reader.clone(), writer.clone()
    );

    let writer_context = ClientContext::new(
        writing_stream, reader, writer
    );

    Ok((reader_context, writer_context))
}

pub fn build_server_connection(
    stream: TcpStream,
    names: NamesMap,
    clients: Clients,
) -> Result<(ServerContext, ServerContext)> {
    let reading_stream = SharedStream::new(stream.try_clone()?);
    let writing_stream = SharedStream::new(stream);

    let reader = SharedStream::new(
        XXsonReader::<_, ClientMessage>::new(reading_stream.clone())
    );

    let writer = SharedStream::new(
        XXsonWriter::<_, ServerMessage>::new(writing_stream.clone())
    );

    let reader_context = ServerContext::new(
        reading_stream, names.clone(), clients.clone(), reader.clone(), writer.clone()
    );

    let writer_context = ServerContext::new(
        writing_stream, names, clients, reader, writer
    );

    Ok((reader_context, writer_context))
}
