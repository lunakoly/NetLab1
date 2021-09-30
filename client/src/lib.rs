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
    explain_common_error,
    MessageProcessing,
};

use chars_reader::{IntoCharsReader, CharsReader};

use std::net::TcpStream;

use std::io::{BufRead};

use std::iter::Peekable;

use commands::{Command, CommandProcessing};

fn handle_server_message(
    reader: &mut XXsonReader<TcpStream, ServerMessage>,
) -> Result<MessageProcessing> {
    let message = match reader.read() {
        Ok(it) => it,
        Err(error) => {
            let explaination = explain_common_error(&error);
            println!("[Server] Error > {}", &explaination);
            return Ok(MessageProcessing::Stop)
        }
    };

    println!("{}", message.visualize(reader)?);
    Ok(MessageProcessing::Proceed)
}

fn handle_server_messages(
    mut reader: XXsonReader<TcpStream, ServerMessage>,
) -> Result<()> {
    loop {
        let result = handle_server_message(&mut reader)?;

        if let MessageProcessing::Stop = &result {
            break
        }
    }

    Ok(())
}

fn handle_user_command(
    writer: &mut XXsonWriter<TcpStream, ClientMessage>,
    reader: &mut Peekable<CharsReader>,
) -> Result<CommandProcessing> {
    match commands::parse(reader) {
        Command::Text { text } => {
            let message = ClientMessage::Text {
                text: text,
            };

            writer.write(&message)?;
        }
        Command::End => {
            return Ok(CommandProcessing::Stop)
        }
        Command::Rename { new_name } => {
            let message = ClientMessage::Rename {
                new_name: new_name,
            };

            writer.write(&message)?;
        }
        Command::Nothing => {}
    }

    Ok(CommandProcessing::Proceed)
}

fn handle_user_commands(
    mut writer: XXsonWriter<TcpStream, ClientMessage>,
) -> Result<()> {
    let stdin = std::io::stdin();
    let lock: &mut dyn BufRead = &mut stdin.lock();
    let mut reader = lock.chars().peekable();

    loop {
        let result = handle_user_command(&mut writer, &mut reader)?;

        if let CommandProcessing::Stop = &result {
            break
        }
    }

    writer.write(&ClientMessage::Leave)?;
    Ok(())
}

fn handle_connection() -> Result<()> {
    let stream = TcpStream::connect("127.0.0.1:6969")?;
    let connection = ClientSideConnection::new(stream)?;
    let reader = connection.reader;

    std::thread::spawn(|| handle_server_messages(reader));

    handle_user_commands(connection.writer)?;
    Ok(())
}

pub fn start() {
    match with_error_report(handle_connection) {
        Ok(_) => println!("Good."),
        Err(_) => println!("Bad."),
    }
}
