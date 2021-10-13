use serde::{Serialize, Deserialize};

use bson::{DateTime};

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    Text { text: String },
    Leave,
    Rename { new_name: String },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    Text { text: String, name: String, time: DateTime },
    NewUser { name: String, time: DateTime },
    Interrupt { name: String, time: DateTime },
    UserLeaves { name: String, time: DateTime },
    Support { text: String },
    UserRenamed { old_name: String, new_name: String },
}
