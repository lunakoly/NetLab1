pub mod connection;
pub mod messages;
pub mod sharers;

use std::io::{Read, Write};
use std::marker::{PhantomData};
use std::fmt::{Display, Formatter};
use std::fs::{File};

use crate::{ErrorKind, Result};
use crate::shared::{Shared};
use crate::communication::bson::{BsonReader, BsonWriter};
use crate::capped_reader::{CAPPED_READER_CAPACITY};

use crate::communication::{
    ReadMessage,
    ReadMessageWithContext,
    WriteMessage,
    WriteMessageWithContext,
};

use connection::{
    ClientContext,
    ServerContext,
    Connection,
};

use messages::{CommonMessage, ClientMessage, ServerMessage};

use bson::doc;

use chrono::{DateTime, Local};

// Found empirically
pub const MINIMUM_TEXT_MESSAGE_SIZE: usize = 52;
pub const MAXIMUM_TEXT_MESSAGE_CONTENT: usize = CAPPED_READER_CAPACITY - MINIMUM_TEXT_MESSAGE_SIZE;

pub const MAXIMUM_TEXT_SIZE: usize = MAXIMUM_TEXT_MESSAGE_CONTENT / 2;
pub const MAXIMUM_NAME_SIZE: usize = MAXIMUM_TEXT_SIZE;

pub const CHUNK_SIZE: usize = 100;

pub struct XXsonReader<R, M, C> {
    backend: BsonReader<R>,
    phantom: PhantomData<M>,
    phantom2: PhantomData<C>,
}

impl<R: Read, M, C> XXsonReader<R, M, C> {
    pub fn new(reader: R) -> XXsonReader<R, M, C> {
        XXsonReader {
            backend: BsonReader::new(reader),
            phantom: PhantomData,
            phantom2: PhantomData,
        }
    }
}

impl<R: Read> XXsonReader<R, ClientMessage, Shared<ServerContext>> {
    fn read_single_message(&mut self) -> Result<ClientMessage> {
        let it = self.backend.read_message()?;
        let message: ClientMessage = bson::from_bson(it.into())?;
        Ok(message)
    }

    fn process_implicitly(
        &mut self,
        mut context: Shared<ServerContext>,
        mut writer: Shared<impl WriteMessageWithContext<CommonMessage, Shared<ServerContext>>>,
    ) -> Result<Option<ClientMessage>> {
        let message = self.read_single_message()?;

        match &message {
            ClientMessage::Common { common } => match common {
                CommonMessage::SendFile { name, size, id } => {
                    context.prepare_sharer(name, name)?;
                    context.promote_sharer(name, size.clone(), id.clone())?;
                }
                CommonMessage::Chunk { data, id } => {
                    let done = context.accept_chunk(data, id.clone())?;

                    if !done {
                        return Ok(None);
                    }

                    let sharer = if let Some(that) = context.remove_sharer(id.clone())? {
                        that
                    } else {
                        return Ok(None)
                    };

                    let notification = ClientMessage::ReceiveFile {
                        name: sharer.name,
                        path: sharer.path,
                    };

                    return Ok(Some(notification))
                }
            }
            ClientMessage::RequestFile { name } => {
                let id = context.free_id()?;

                let mut file = File::open(name)?;
                let size = file.metadata()?.len() as usize;

                writer.send_file(context, &mut file, name, size, id)?;
            }
            _ => {
                return Ok(Some(message))
            }
        }

        Ok(None)
    }
}

impl<R, W> ReadMessageWithContext<
    ClientMessage,
    Shared<ServerContext>,
    Shared<W>
> for XXsonReader<
    R,
    ClientMessage,
    Shared<ServerContext>
> where
    R: Read,
    W: WriteMessageWithContext<CommonMessage, Shared<ServerContext>>,
{
    fn read_message_with_context(
        &mut self,
        context: Shared<ServerContext>,
        writer: Shared<W>,
    ) -> Result<ClientMessage> {
        let mut it = self.process_implicitly(context.clone(), writer.clone())?;

        while let None = it {
            it = self.process_implicitly(context.clone(), writer.clone())?;
        }

        Ok(it.unwrap())
    }
}

impl<R: Read> XXsonReader<R, ServerMessage, Shared<ClientContext>> {
    fn read_single_message(&mut self) -> Result<ServerMessage> {
        let it = self.backend.read_message()?;
        let message: ServerMessage = bson::from_bson(it.into())?;
        Ok(message)
    }

    fn process_implicitly(
        &mut self,
        mut context: Shared<ClientContext>,
        _: Shared<impl WriteMessageWithContext<CommonMessage, Shared<ClientContext>>>
    ) -> Result<Option<ServerMessage>> {
        let message = self.read_single_message()?;

        match &message {
            ServerMessage::Common { common } => match common {
                CommonMessage::SendFile { name, size, id } => {
                    context.promote_sharer(name, size.clone(), id.clone())?;
                }
                CommonMessage::Chunk { data, id } => {
                    let done = context.accept_chunk(data, id.clone())?;

                    if !done {
                        return Ok(None);
                    }

                    let sharer = if let Some(that) = context.remove_sharer(id.clone())? {
                        that
                    } else {
                        return Ok(None)
                    };

                    let notification = ServerMessage::ReceiveFile {
                        name: sharer.name,
                        path: sharer.path,
                    };

                    return Ok(Some(notification))
                }
            }
            _ => {
                return Ok(Some(message))
            }
        }

        Ok(None)
    }
}

impl<R, W> ReadMessageWithContext<
    ServerMessage,
    Shared<ClientContext>,
    Shared<W>,
> for XXsonReader<
    R,
    ServerMessage,
    Shared<ClientContext>
> where
    R: Read,
    W: WriteMessageWithContext<CommonMessage, Shared<ClientContext>>,
{
    fn read_message_with_context(
        &mut self,
        context: Shared<ClientContext>,
        writer: Shared<W>,
    ) -> Result<ServerMessage> {
        let mut it = self.process_implicitly(context.clone(), writer.clone())?;

        while let None = it {
            it = self.process_implicitly(context.clone(), writer.clone())?;
        }

        Ok(it.unwrap())
    }
}

pub struct XXsonWriter<W, M, C> {
    backend: BsonWriter<W>,
    phantom: PhantomData<M>,
    phantom2: PhantomData<C>,
}

impl<W, M, C> XXsonWriter<W, M, C> {
    pub fn new(stream: W) -> XXsonWriter<W, M, C> {
        XXsonWriter {
            backend: BsonWriter::new(stream),
            phantom: PhantomData,
            phantom2: PhantomData,
        }
    }
}

impl<W: Write> XXsonWriter<W, ClientMessage, Shared<ClientContext>> {
    fn write_single_message(&mut self, message: &ClientMessage) -> Result<()> {
        let serialized = bson::to_bson(message)?;

        if let Some(it) = serialized.as_document() {
            self.backend.write_message(it)
        } else if let Some(it) = serialized.as_str() {
            let wrapper = doc! {
                it: {}
            };

            self.backend.write_message(&wrapper)
        } else {
            Err(ErrorKind::NothingToRead.into())
        }
    }

    fn process_implicitly(
        &mut self,
        message: &ClientMessage,
        mut context: Shared<ClientContext>,
    ) -> Result<()> {
        match message {
            ClientMessage::UploadFile { name, path } => {
                let id = context.free_id()?;

                let mut file = File::open(path)?;
                let size = file.metadata()?.len() as usize;

                self.send_file(context, &mut file, name, size, id)?;
            }
            ClientMessage::DownloadFile { name, path } => {
                context.prepare_sharer(path, name)?;

                let inner = ClientMessage::RequestFile {
                    name: name.clone(),
                };

                self.write_single_message(&inner)?;
            }
            _ => {
                self.write_single_message(message)?;
            }
        }

        Ok(())
    }
}

impl<W: Write> WriteMessageWithContext<
    ClientMessage,
    Shared<ClientContext>
> for XXsonWriter<
    W,
    ClientMessage,
    Shared<ClientContext>
> {
    fn write_message_with_context(
        &mut self,
        message: &ClientMessage,
        context: Shared<ClientContext>,
    ) -> Result<()> {
        self.process_implicitly(message, context)
    }
}

impl<W: Write> WriteMessageWithContext<
    CommonMessage,
    Shared<ClientContext>,
> for XXsonWriter<
    W,
    ClientMessage,
    Shared<ClientContext>,
> {
    fn write_message_with_context(
        &mut self,
        message: &CommonMessage,
        _: Shared<ClientContext>,
    ) -> Result<()> {
        let document = ClientMessage::Common {
            common: message.clone(),
        };

        self.write_single_message(&document)
    }
}

impl<W: Write> XXsonWriter<W, ServerMessage, Shared<ServerContext>> {
    fn write_single_message(&mut self, message: &ServerMessage) -> Result<()> {
        let serialized = bson::to_bson(message)?;

        if let Some(it) = serialized.as_document() {
            self.backend.write_message(it)
        } else if let Some(it) = serialized.as_str() {
            let wrapper = doc! {
                it: {}
            };

            self.backend.write_message(&wrapper)
        } else {
            Err(ErrorKind::NothingToRead.into())
        }
    }

    fn process_implicitly(
        &mut self,
        message: &ServerMessage,
        _: Shared<ServerContext>,
    ) -> Result<()> {
        self.write_single_message(message)
    }
}

impl<W: Write> WriteMessageWithContext<
    ServerMessage,
    Shared<ServerContext>,
> for XXsonWriter<
    W,
    ServerMessage,
    Shared<ServerContext>,
> {
    fn write_message_with_context(
        &mut self,
        message: &ServerMessage,
        context: Shared<ServerContext>,
    ) -> Result<()> {
        self.process_implicitly(message, context)
    }
}

impl<W: Write> WriteMessageWithContext<
    CommonMessage,
    Shared<ServerContext>,
> for XXsonWriter<
    W,
    ServerMessage,
    Shared<ServerContext>
> {
    fn write_message_with_context(
        &mut self,
        message: &CommonMessage,
        _: Shared<ServerContext>,
    ) -> Result<()> {
        let document = ServerMessage::Common {
            common: message.clone(),
        };

        self.write_single_message(&document)
    }
}

trait SendFile<C> {
    fn send_file(
        &mut self,
        context: C,
        file: &mut File,
        name: &str,
        size: usize,
        id: usize
    ) -> Result<()>;
}

impl<C, W> SendFile<C> for W
where
    W: WriteMessageWithContext<CommonMessage, C>,
    C: Clone,
{
    fn send_file(
        &mut self,
        context: C,
        file: &mut File,
        name: &str,
        size: usize,
        id: usize
    ) -> Result<()> {
        let mut written = 0usize;

        let prelude = CommonMessage::SendFile {
            name: name.to_owned(),
            size: size,
            id: id.clone(),
        };

        self.write_message_with_context(&prelude, context.clone())?;

        while written < size {
            let mut buffer = [0u8; CHUNK_SIZE];
            let read = file.read(&mut buffer)?;
            written += read;

            let chunk = CommonMessage::Chunk {
                data: buffer.to_vec(),
                id: id.clone(),
            };

            self.write_message_with_context(&chunk, context.clone())?;
        }

        Ok(())
    }
}

impl Display for ServerMessage {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self {
            ServerMessage::Text { text, name, time } => {
                let the_time: DateTime<Local> = time.to_chrono().into();
                let formatted = the_time.format("%e %b %Y %T");
                write!(formatter, "<{}> [{}] {}", formatted, name, text)
            }
            ServerMessage::NewUser { name, .. } => {
                write!(formatter, "~~ Meet our new mate: {} ~~", name)
            }
            ServerMessage::Interrupt { name, .. } => {
                write!(formatter, "~~ Press F, {} ~~", name)
            }
            ServerMessage::UserLeaves { name, .. } => {
                write!(formatter, "~~ {} leaves the party ~~", name)
            }
            ServerMessage::Support { text } => {
                write!(formatter, "(Server) {}", &text)
            }
            ServerMessage::UserRenamed { old_name, new_name } => {
                write!(formatter, "~~ He once used to be {}, but now he is {} ~~", &old_name, &new_name)
            }
            ServerMessage::NewFile { name } => {
                write!(formatter, "~~ And the new file is {} ~~", &name)
            }
            ServerMessage::ReceiveFile { name, path } => {
                write!(formatter, "(Console) Downloaded {} and saved to {}", &name, &path)
            }
            ServerMessage::DeclineFile { reason } => {
                write!(formatter, "(Server) Nah, wait. {}", &reason)
            }
            ServerMessage::Common { common } => match common {
                CommonMessage::SendFile { name, size, id } => {
                    write!(formatter, "(Server) Get ready for {} bytes with tag #{} (for {})", &size, id, name)
                }
                CommonMessage::Chunk { .. } => {
                    write!(formatter, "(Server) Here are some bytes for you")
                }
            }
        }
    }
}
