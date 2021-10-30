use std::fmt::{Display, Formatter};

use chrono::{Local};

use serde::{Serialize, Deserialize};

use bson::{DateTime};

pub const MAXIMUM_MESSAGE_SIZE: usize = 1024;

// Found empirically
pub const MINIMUM_TEXT_MESSAGE_SIZE: usize = 52;
pub const MAXIMUM_TEXT_MESSAGE_CONTENT: usize = MAXIMUM_MESSAGE_SIZE - MINIMUM_TEXT_MESSAGE_SIZE;

pub const MAXIMUM_TEXT_SIZE: usize = MAXIMUM_TEXT_MESSAGE_CONTENT / 2;
pub const MAXIMUM_NAME_SIZE: usize = MAXIMUM_TEXT_SIZE;

pub const MINIMUM_CLIENT_REQUEST_FILE_UPLOAD_SIZE: usize = 66;
pub const MAXIMUM_FILE_NAME_SIZE: usize = MAXIMUM_MESSAGE_SIZE - MINIMUM_CLIENT_REQUEST_FILE_UPLOAD_SIZE;

pub const CHUNK_SIZE: usize = 100;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CommonMessage {
    Chunk { data: Vec<u8>, id: usize },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    // Main
    Text { text: String },
    Leave,
    Rename { new_name: String },

    // Sending files
    Common { common: CommonMessage },
    RequestFileUpload { name: String, size: usize, id: usize },
    RequestFileDownload { name: String },
    AgreeFileDownload { id: usize },
    DeclineFileDownload { id: usize },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    // Main
    Text { text: String, name: String, time: DateTime },
    NewUser { name: String, time: DateTime },
    Interrupt { name: String, time: DateTime },
    UserLeaves { name: String, time: DateTime },
    Support { text: String },
    UserRenamed { old_name: String, new_name: String },
    NewFile { name: String },

    // Sending files
    Common { common: CommonMessage },
    AgreeFileUpload { id: usize },
    DeclineFileUpload { id: usize, reason: String },
    AgreeFileDownload { name: String, size: usize, id: usize },
    DeclineFileDownload { name: String, reason: String },
}

impl Display for ServerMessage {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self {
            ServerMessage::Text { text, name, time } => {
                let the_time: chrono::DateTime<Local> = time.to_chrono().into();
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
