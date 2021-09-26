mod chars_reader;
mod commands;

use shared::{Result, Error, with_error_report};

use shared::communication::xxson::{
    ClientSideConnection,
    ClientMessage,
    Visualize,
};

use shared::communication::{
    ReadMessage,
    WriteMessage,
    try_explain_common_error,
};

use chars_reader::{IntoCharsReader};

use std::net::TcpStream;

use std::io::{BufRead};

fn handle_error(error: &Error) {
    println!("Error > {}", error);
}

fn handle_input(connection: &mut ClientSideConnection) -> Result<()> {
    match connection.reader.read() {
        Ok(value) => {
            value.visualize(connection)?;
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

fn handle_connection() -> Result<()> {
    let stream = TcpStream::connect("127.0.0.1:6969")?;
    let mut connection = ClientSideConnection::new(stream)?;

    let stdin = std::io::stdin();
    let lock: &mut dyn BufRead = &mut stdin.lock();
    let mut reader = lock.chars().peekable();

    loop {
        match commands::parse(&mut reader) {
            commands::Command::Message { text } => {
                let message = ClientMessage::Text {
                    text: text,
                };

                connection.writer.write(&message)?;
                handle_input(&mut connection)?;
            }
            commands::Command::End => {
                break
            }
            _ => {}
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
