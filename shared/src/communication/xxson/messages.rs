use serde::{Serialize, Deserialize};

use bson::{DateTime};

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
