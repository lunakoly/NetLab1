use serde::{Serialize, Deserialize};

use bson::{DateTime};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CommonMessage {
    // The client never creates these manually,
    // and the server never reads these
    // SendFile { name: String, size: usize, id: usize },
    Chunk { data: Vec<u8>, id: usize },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    // The client creates these manually,
    // and the server reads these messages
    Text { text: String },
    Leave,
    Rename { new_name: String },

    // The client creates these manually,
    // but the server never reads these
    UploadFile { name: String, path: String },
    DownloadFile { name: String, path: String },

    // The client never creates these manually,
    // and the server never reads these
    Common { common: CommonMessage },
    RequestFileUpload { name: String, size: usize, id: usize },
    RequestFileDownload { name: String },
    AgreeFileDownload { id: usize },
    DeclineFileDownload { id: usize },

    // The client never creates these manually,
    // but the server reads these
    ReceiveFile { name: String, path: String },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    // The server creates these manually,
    // and the client reads these messages
    Text { text: String, name: String, time: DateTime },
    NewUser { name: String, time: DateTime },
    Interrupt { name: String, time: DateTime },
    UserLeaves { name: String, time: DateTime },
    Support { text: String },
    UserRenamed { old_name: String, new_name: String },
    NewFile { name: String },

    // The server never creates these manually,
    // and the client never reads these
    Common { common: CommonMessage },
    AgreeFileUpload { id: usize },
    DeclineFileUpload { id: usize, reason: String },
    AgreeFileDownload { name: String, size: usize, id: usize },
    DeclineFileDownload { name: String, reason: String },

    // The server never creates these manually,
    // but the client reads these
    ReceiveFile { name: String, path: String },
    DeclineFileUpload2 { name: String, reason: String },
    DeclineFileDownload2 { name: String, reason: String },
}
