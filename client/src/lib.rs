mod chars_reader;
mod commands;

use shared::{Result, with_error_report};

use shared::communication::xxson::{
    ClientSideConnection,
    ClientMessage,
    ServerMessage,
    VisualizeServerMessage,
    XXsonReader,
    XXsonWriter,
};

use shared::communication::{
    ReadMessage,
    WriteMessage,
    try_explain_common_error,
};

use chars_reader::{IntoCharsReader};

use std::net::TcpStream;

use std::io::{BufRead};

fn handle_server_messages(
    mut reader: XXsonReader<TcpStream, ServerMessage>,
) -> Result<()> {
    match reader.read() {
        Ok(value) => {
            value.visualize(&reader)?;
            Ok(())
        }
        Err(error) => {
            let prefix = "[Server] ";

            if try_explain_common_error(&error, prefix) {
                println!("{}Error > {}", prefix, error);
            }

            Err(error)
        }
    }
}

fn handle_user_input(
    mut writer: XXsonWriter<TcpStream, ClientMessage>,
) -> Result<()> {
    let stdin = std::io::stdin();
    let lock: &mut dyn BufRead = &mut stdin.lock();
    let mut reader = lock.chars().peekable();

    loop {
        match commands::parse(&mut reader) {
            commands::Command::Message { text } => {
                let message = ClientMessage::Text {
                    text: text,
                };

                writer.write(&message)?;
            }
            commands::Command::End => {
                break
            }
            _ => {}
        }
    }

    Ok(())
}

fn handle_connection() -> Result<()> {
    let stream = TcpStream::connect("127.0.0.1:6969")?;
    let connection = ClientSideConnection::new(stream)?;
    let reader = connection.reader;

    std::thread::spawn(|| -> Result<()> {
        handle_server_messages(reader)?;
        Ok(())
    });

    handle_user_input(connection.writer)?;
    Ok(())
}

pub fn start() {
    match with_error_report(handle_connection) {
        Ok(_) => println!("Good."),
        Err(_) => println!("Bad."),
    }
}
