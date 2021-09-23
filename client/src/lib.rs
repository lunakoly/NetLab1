mod chars_reader;
mod commands;

use shared::{Result, Error, with_error_report};
use shared::connection::Connection;
use shared::communication::json::visualize;

use shared::communication::{
    ReadMessage, 
    WriteMessage, 
    try_explain_common_error,
    dictionary,
};

use dictionary::{
    TYPE,
    MESSAGE,
    TEXT,
    NAME,
};

use chars_reader::{IntoCharsReader};

use std::net::TcpStream;

use std::io::{BufRead};

use serde_json::json;

pub fn handle_error(error: &Error) {
    println!("Error > {}", error);
}

pub fn handle_input(connection: &mut Connection) -> Result<()> {
    match connection.reader.read() {
        Ok(value) => {
            visualize(&value, connection)?;
            Ok(())
        }
        Err(error) => {
            if try_explain_common_error(&error) {
                handle_error(&error);
            }

            Err(error)
        }
    }
}

pub fn handle_connection() -> Result<()> {
    let stream = TcpStream::connect("127.0.0.1:6969")?;
    let mut connection = shared::connection::prepare(stream)?;

    let stdin = std::io::stdin();
    let lock: &mut dyn BufRead = &mut stdin.lock();
    let mut reader = lock.chars().peekable();

    loop {
        match commands::parse(&mut reader) {
            commands::Command::Message { text } => {
                let message = json!({
                    TYPE: MESSAGE,
                    NAME: "Nick",
                    TEXT: text
                });
            
                connection.writer.write(&message)?;
                handle_input(&mut connection)?;
            }
            _ => {
                break
            }
        }
    }

    Ok(())
}

pub fn start() {
    match with_error_report(handle_connection) {
        Ok(_) => println!("Good."),
        Err(_) => println!("Bad."),
    }
}
