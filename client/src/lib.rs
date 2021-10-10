mod chars_reader;
mod commands;

use std::io::{BufRead};
use std::iter::Peekable;
use std::cell::{RefCell};
use std::net::{TcpStream};

use shared::{Result, with_error_report};
use shared::helpers::{TcpSplit};

use shared::communication::xxson::{
    ClientMessage,
};

use shared::communication::xxson::connection::{
    ClientReadingConnection,
    ClientWritingConnection,
    ClientReadingContext,
    ClientWritingContext,
};

use shared::communication::{
    WriteMessage,
    explain_common_error,
    MessageProcessing,
};

use chars_reader::{IntoCharsReader, CharsReader};
use commands::{Command, CommandProcessing};

fn handle_server_message(
    connection: &mut impl ClientReadingConnection
) -> Result<MessageProcessing> {
    let message = match connection.read() {
        Ok(it) => it,
        Err(error) => {
            let explaination = explain_common_error(&error);
            println!("(Server) Error > {}", &explaination);
            return Ok(MessageProcessing::Stop)
        }
    };

    println!("{}", message);
    Ok(MessageProcessing::Proceed)
}

fn handle_server_messages(stream: RefCell<TcpStream>) -> Result<()> {
    let mut connection = ClientReadingContext::new(&stream);

    loop {
        let result = handle_server_message(&mut connection)?;

        if let MessageProcessing::Stop = &result {
            break
        }
    }

    Ok(())
}

fn match_user_command_with_connection(
    command: Command,
    connection: &mut impl ClientWritingConnection,
) -> Result<CommandProcessing> {
    match command {
        Command::Text { text } => {
            let message = ClientMessage::Text {
                text: text,
            };

            connection.write(&message)?;
        }
        Command::Rename { new_name } => {
            let message = ClientMessage::Rename {
                new_name: new_name,
            };

            connection.write(&message)?;
        }
        _ => {}
    }

    Ok(CommandProcessing::Proceed)
}

fn handle_user_command(
    connection: &mut Option<impl ClientWritingConnection>,
    reader: &mut Peekable<CharsReader>,
) -> Result<CommandProcessing> {
    match commands::parse(reader) {
        Command::End => {
            return Ok(CommandProcessing::Stop)
        }
        Command::Connect { address } => {
            let (writing_stream, reading_stream) = TcpStream::connect(address)?
                .split_to_refcells()?;

            std::thread::spawn(|| {
                with_error_report(|| handle_server_messages(reading_stream))
            });

            return Ok(CommandProcessing::Connect(writing_stream))
        }
        Command::Nothing => {}
        other => match connection {
            Some(it) => {
                match_user_command_with_connection(other, it)?;
            }
            None => {
                println!("(Console) Easy now! We should first establish a connection, all right? Go on, use /connect");
            }
        }
    }

    Ok(CommandProcessing::Proceed)
}

fn handle_user_commands() -> Result<()> {
    let stdin = std::io::stdin();
    let lock: &mut dyn BufRead = &mut stdin.lock();
    let mut reader = lock.chars().peekable();

    let mut writing_stream: RefCell<TcpStream>;
    let mut connection: Option<ClientWritingContext> = None;

    loop {
        let result = handle_user_command(&mut connection, &mut reader)?;

        if let CommandProcessing::Stop = &result {
            break
        } else if let CommandProcessing::Connect(stream) = result {
            writing_stream = stream;
            connection = Some(ClientWritingContext::new(&writing_stream));
        }
    }

    if let Some(it) = &mut connection {
        it.write(&ClientMessage::Leave)?;
    }

    Ok(())
}

fn handle_connection() -> Result<()> {
    handle_user_commands()
}

pub fn start() {
    match with_error_report(handle_connection) {
        Ok(_) => println!("Good."),
        Err(_) => println!("Bad."),
    }
}
