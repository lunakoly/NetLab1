use std::net::TcpListener;
use std::io::prelude::Write;

use shared::{Result, Error, with_error_report};
use shared::communication::bson::visualize;
use shared::connection::Connection;

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

use bson::doc;

pub fn handle_error(
    connection: &mut Connection,
    error: &Error,
) {
    let response = "HTTP/1.1 404 OK\r\n\r\n";

    connection.writer.stream.write(response.as_bytes()).unwrap();
    connection.writer.stream.flush().unwrap();

    println!("Error > {}", error);
}

pub fn handle_input(connection: &mut Connection) -> Result<()> {
    match connection.reader.read() {
        Ok(value) => {
            visualize(&value, connection)?;

            let response = doc! {
                TYPE: MESSAGE,
                NAME: "Server", 
                TEXT: "Hi, got it."
            };

            connection.writer.write(&response)?;

            Ok(())
        }
        Err(error) => {
            if try_explain_common_error(&error) {
                handle_error(connection, &error);
            }

            Err(error)
        }
    }
}

pub fn handle_connection() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6969")?;

    for incomming in listener.incoming() {
        let stream = incomming?;
        let mut connection = shared::connection::prepare(stream)?;

        loop {
            if let Err(..) = handle_input(&mut connection) {
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
