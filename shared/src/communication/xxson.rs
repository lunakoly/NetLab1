pub mod connection;
pub mod messages;
pub mod sharers;

use std::io::{Read, Write};
use std::marker::{PhantomData};
use std::fmt::{Display, Formatter};
use std::path::{Path};
use std::fs::{File};

use crate::{Result};
use crate::shared::{Shared};
use crate::errors::{with_error_report};
use crate::communication::arson::{ArsonReader, ArsonWriter};
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

use chrono::{DateTime, Local};

// Found empirically
pub const MINIMUM_TEXT_MESSAGE_SIZE: usize = 52;
pub const MAXIMUM_TEXT_MESSAGE_CONTENT: usize = CAPPED_READER_CAPACITY - MINIMUM_TEXT_MESSAGE_SIZE;

pub const MAXIMUM_TEXT_SIZE: usize = MAXIMUM_TEXT_MESSAGE_CONTENT / 2;
pub const MAXIMUM_NAME_SIZE: usize = MAXIMUM_TEXT_SIZE;

pub const CHUNK_SIZE: usize = 100;

pub struct XXsonReader<R, C> {
    backend: ArsonReader<R>,
    phantom: PhantomData<C>,
}

impl<R: Read, C> XXsonReader<R, C> {
    pub fn new(reader: R) -> XXsonReader<R, C> {
        XXsonReader {
            backend: ArsonReader::new(reader),
            phantom: PhantomData,
        }
    }
}

impl<R: Read> XXsonReader<R, ServerContext> {
    fn process_implicitly<W>(
        &mut self,
        mut context: Shared<ServerContext>,
        mut writer: Shared<W>,
    ) -> Result<Option<ClientMessage>>
    where
        W: WriteMessageWithContext<CommonMessage, Shared<ServerContext>>
         + WriteMessageWithContext<ServerMessage, Shared<ServerContext>>
         + Send + Sync + 'static,
    {
        let message = self.backend.read_message()?;

        match &message {
            ClientMessage::Common { common } => match common {
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
            ClientMessage::RequestFileUpload { name, size, id } => {
                let response = if Path::new(name).exists() {
                    ServerMessage::DeclineFileUpload {
                        id: id.clone(),
                        reason: "There's already a file with such a name".to_owned(),
                    }
                } else {
                    context.prepare_sharer(name, File::create(name)?, name)?;
                    context.promote_sharer(name, size.clone(), id.clone())?;

                    ServerMessage::AgreeFileUpload {
                        id: id.clone(),
                    }
                };

                writer.write_message_with_context(&response, context)?;
            }
            ClientMessage::RequestFileDownload { name } => {
                let response = if !Path::new(name).exists() {
                    ServerMessage::DeclineFileDownload {
                        name: name.clone(),
                        reason: "There's no such a file".to_owned(),
                    }
                } else {
                    let id = context.free_id()?;

                    let file = File::open(name)?;
                    let size = file.metadata()?.len() as usize;

                    context.prepare_sharer(name, file, name)?;
                    context.promote_sharer(name, size.clone(), id.clone())?;

                    ServerMessage::AgreeFileDownload {
                        name: name.clone(),
                        id: id.clone(),
                        size: size,
                    }
                };

                writer.write_message_with_context(&response, context)?;
            }
            ClientMessage::AgreeFileDownload { id } => {
                let file: File;
                let size: usize;

                {
                    let sharers = context.sharers_map()?;
                    let mut locked = sharers.write()?;
                    let key = format!("{}", id);

                    let sharer = if let Some(it) = locked.get_mut(&key) {
                        it
                    } else {
                        return Ok(None)
                    };

                    file = sharer.file.try_clone()?;
                    size = sharer.size.clone();
                }

                writer.send_file_non_blocking(context, file, size, id.clone())?;
            }
            ClientMessage::DeclineFileDownload { id } => {
                // Well, they asked for the file, but
                // now they say they can't accept the size.
                context.remove_sharer(id.clone())?;
                return Ok(None)
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
    ServerContext
> where
    R: Read,
    W: WriteMessageWithContext<CommonMessage, Shared<ServerContext>>
     + WriteMessageWithContext<ServerMessage, Shared<ServerContext>>
     + Send + Sync + 'static,
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

impl<R: Read> XXsonReader<R, ClientContext> {
    fn process_implicitly<W>(
        &mut self,
        mut context: Shared<ClientContext>,
        mut writer: Shared<W>
    ) -> Result<Option<ServerMessage>>
    where
        W: WriteMessageWithContext<CommonMessage, Shared<ClientContext>>
         + WriteMessageWithContext<ClientMessage, Shared<ClientContext>>
         + Send + Sync + 'static,
    {
        let message = self.backend.read_message()?;

        match &message {
            ServerMessage::Common { common } => match common {
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
            ServerMessage::AgreeFileUpload { id } => {
                let file: File;
                let size: usize;

                {
                    let sharers = context.sharers_map()?;
                    let mut locked = sharers.write()?;
                    let key = format!("{}", id);

                    let sharer = if let Some(it) = locked.get_mut(&key) {
                        it
                    } else {
                        return Ok(None)
                    };

                    file = sharer.file.try_clone()?;
                    size = sharer.size.clone();
                }

                writer.send_file_non_blocking(context, file, size, id.clone())?;
            }
            ServerMessage::DeclineFileUpload { id, reason } => {
                let sharer = if let Some(it) = context.remove_sharer(id.clone())? {
                    it
                } else {
                    return Ok(None)
                };

                let forwarded = ServerMessage::DeclineFileUpload2 {
                    name: sharer.name,
                    reason: reason.clone(),
                };

                return Ok(Some(forwarded))
            }
            ServerMessage::AgreeFileDownload { name, size, id } => {
                context.promote_sharer(name, size.clone(), id.clone())?;

                let response = ClientMessage::AgreeFileDownload {
                    id: id.clone(),
                };

                writer.write_message_with_context(&response, context)?;
            }
            ServerMessage::DeclineFileDownload { name, reason } => {
                context.remove_unpromoted_sharer(name)?;

                let forwarded = ServerMessage::DeclineFileDownload2 {
                    name: name.clone(),
                    reason: reason.clone(),
                };

                return Ok(Some(forwarded))
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
    ClientContext
> where
    R: Read,
    W: WriteMessageWithContext<CommonMessage, Shared<ClientContext>>
     + WriteMessageWithContext<ClientMessage, Shared<ClientContext>>
     + Send + Sync + 'static,
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

pub struct XXsonWriter<W, C> {
    backend: ArsonWriter<W>,
    phantom: PhantomData<C>,
}

impl<W: Write, C> XXsonWriter<W, C> {
    pub fn new(stream: W) -> XXsonWriter<W, C> {
        XXsonWriter {
            backend: ArsonWriter::new(stream),
            phantom: PhantomData,
        }
    }
}

impl<W: Write> XXsonWriter<W, ClientContext> {
    fn process_implicitly(
        &mut self,
        message: &ClientMessage,
        mut context: Shared<ClientContext>,
    ) -> Result<()> {
        match message {
            ClientMessage::UploadFile { name, path } => {
                let id = context.free_id()?;

                let file = File::open(path)?;
                let size = file.metadata()?.len() as usize;

                let request = ClientMessage::RequestFileUpload {
                    name: name.clone(),
                    size: size,
                    id: id,
                };

                context.prepare_sharer(path, file, name)?;
                context.promote_sharer(name, size, id)?;

                self.write_message_with_context(&request, context)?;
            }
            ClientMessage::DownloadFile { name, path } => {
                context.prepare_sharer(path, File::create(path)?, name)?;

                let inner = ClientMessage::RequestFileDownload {
                    name: name.clone(),
                };

                self.backend.write_message(&inner)?;
            }
            _ => {
                self.backend.write_message(message)?;
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
    ClientContext
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
    ClientContext,
> {
    fn write_message_with_context(
        &mut self,
        message: &CommonMessage,
        _: Shared<ClientContext>,
    ) -> Result<()> {
        let document = ClientMessage::Common {
            common: message.clone(),
        };

        self.backend.write_message(&document)
    }
}

impl<W: Write> XXsonWriter<W, ServerContext> {
    fn process_implicitly(
        &mut self,
        message: &ServerMessage,
        _: Shared<ServerContext>,
    ) -> Result<()> {
        self.backend.write_message(message)
    }
}

impl<W: Write> WriteMessageWithContext<
    ServerMessage,
    Shared<ServerContext>,
> for XXsonWriter<
    W,
    ServerContext,
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
    ServerContext
> {
    fn write_message_with_context(
        &mut self,
        message: &CommonMessage,
        _: Shared<ServerContext>,
    ) -> Result<()> {
        let document = ServerMessage::Common {
            common: message.clone(),
        };

        self.backend.write_message(&document)
    }
}

trait SendFile<C> {
    fn send_file(
        &mut self,
        context: C,
        file: &mut File,
        size: usize,
        id: usize
    ) -> Result<()>;

    fn send_file_non_blocking(
        &mut self,
        context: C,
        file: File,
        size: usize,
        id: usize
    ) -> Result<()>;
}

impl<C, W> SendFile<C> for W
where
    W: WriteMessageWithContext<CommonMessage, C>
     + Clone
     + Send + Sync + 'static,
    C: Clone
     + Send + Sync + 'static,
{
    fn send_file(
        &mut self,
        context: C,
        file: &mut File,
        size: usize,
        id: usize
    ) -> Result<()> {
        let mut written = 0usize;
        let mut old_time_point = Local::now();

        while written < size {
            let mut buffer = [0u8; CHUNK_SIZE];
            let read = file.read(&mut buffer)?;
            written += read;

            let chunk = CommonMessage::Chunk {
                data: buffer.to_vec(),
                id: id.clone(),
            };

            self.write_message_with_context(&chunk, context.clone())?;

            let time_point = Local::now();

            if (time_point - old_time_point).num_seconds() >= 1 {
                old_time_point = time_point;
                println!("(Console) File #{} > {}%", id.clone(), written * 100 / size);
            }
        }

        Ok(())
    }

    fn send_file_non_blocking(
        &mut self,
        context: C,
        file: File,
        size: usize,
        id: usize
    ) -> Result<()> {
        let the_writer = self.clone();

        std::thread::spawn(move || {
            let mut owned_file = file;
            let mut owned_writer = the_writer;

            with_error_report(|| -> Result<()> {
                owned_writer.send_file(context, &mut owned_file, size, id)
            });
        });

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
            ServerMessage::AgreeFileUpload { id } => {
                write!(formatter, "(Server) Sure, I'm ready to accept #{}", &id)
            }
            ServerMessage::DeclineFileUpload { id, reason } => {
                write!(formatter, "(Server) Nah, wait with your #{}. {}", &id, &reason)
            }
            ServerMessage::AgreeFileDownload { name, size, id } => {
                write!(formatter, "(Server) Sure, I'm ready to give you {} ({} bytes, #{})", &name, &size, &id)
            }
            ServerMessage::DeclineFileDownload { name, reason } => {
                write!(formatter, "(Server) Nah, I won't give you {}. {}", &name, &reason)
            }
            ServerMessage::DeclineFileUpload2 { name, reason } => {
                write!(formatter, "(Server) Nah, wait with your #{}. {}", &name, &reason)
            }
            ServerMessage::DeclineFileDownload2 { name, reason } => {
                write!(formatter, "(Server) Nah, I won't give you {}. {}", &name, &reason)
            }
            ServerMessage::Common { common } => match common {
                CommonMessage::Chunk { .. } => {
                    write!(formatter, "(Server) Here are some bytes for you")
                }
            }
        }
    }
}
