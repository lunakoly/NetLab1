pub mod connection;
pub mod messages;
pub mod sharers;

use std::io::{Read};
use std::fs::{File};
use std::fmt::{Display, Formatter};

use crate::{Result};
use crate::errors::{with_error_report};
use crate::capped_reader::{CAPPED_READER_CAPACITY};

use crate::communication::{
    WriteMessage,
    SendFile,
};

use messages::{CommonMessage, ServerMessage};

use chrono::{DateTime, Local};

// Found empirically
pub const MINIMUM_TEXT_MESSAGE_SIZE: usize = 52;
pub const MAXIMUM_TEXT_MESSAGE_CONTENT: usize = CAPPED_READER_CAPACITY - MINIMUM_TEXT_MESSAGE_SIZE;

pub const MAXIMUM_TEXT_SIZE: usize = MAXIMUM_TEXT_MESSAGE_CONTENT / 2;
pub const MAXIMUM_NAME_SIZE: usize = MAXIMUM_TEXT_SIZE;

pub const CHUNK_SIZE: usize = 100;

impl<W> SendFile for W
where
    W: WriteMessage<CommonMessage>
     + Clone
     + Send + Sync + 'static,
{
    fn send_file(
        &mut self,
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

            self.write_message(&chunk)?;

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
        file: File,
        size: usize,
        id: usize
    ) -> Result<()> {
        let the_writer = self.clone();

        std::thread::spawn(move || {
            let mut owned_file = file;
            let mut owned_writer = the_writer;

            with_error_report(|| -> Result<()> {
                owned_writer.send_file(&mut owned_file, size, id)
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
            ServerMessage::Common { common } => match common {
                CommonMessage::Chunk { .. } => {
                    write!(formatter, "(Server) Here are some bytes for you")
                }
            }
        }
    }
}
