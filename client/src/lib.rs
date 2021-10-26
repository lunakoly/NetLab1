mod chars_reader;
mod commands;

use std::fs::{File};
use std::io::{BufRead};
use std::iter::Peekable;
use std::net::{TcpStream};

use shared::{Result, with_error_report};

use shared::communication::xxson::messages::{
    CommonMessage,
    ClientMessage,
    ServerMessage,
};
use shared::communication::xxson::connection::{
    ClientSessionData,
    ClientSession,
    build_client_connection,
};

use shared::communication::{
    WriteMessage,
    SendFile,
    explain_common_error,
    MessageProcessing,
};

use chars_reader::{IntoCharsReader, CharsReader};
use commands::{Command, CommandProcessing};

fn handle_server_message(
    connection: &mut (impl ClientSession + 'static)
) -> Result<MessageProcessing> {
    let message = match connection.read_message() {
        Ok(it) => it,
        Err(error) => {
            let explaination = explain_common_error(&error);
            println!("(Server) Error > {}", &explaination);
            return Ok(MessageProcessing::Stop)
        }
    };

    match message {
        ServerMessage::Common { common } => match common {
            CommonMessage::Chunk { data, id } => {
                let done = connection.accept_chunk(&data, id.clone())?;

                if !done {
                    return Ok(MessageProcessing::Proceed);
                }

                let sharer = if let Some(that) = connection.remove_sharer(id.clone())? {
                    that
                } else {
                    return Ok(MessageProcessing::Proceed)
                };

                println!("(Console) Downloaded {} and saved to {}", &sharer.name, &sharer.path)
            }
        }
        ServerMessage::AgreeFileUpload { id } => {
            let file: File;
            let size: usize;

            {
                let sharers = connection.sharers_map()?;
                let mut locked = sharers.write()?;
                let key = format!("{}", id);

                let sharer = if let Some(it) = locked.get_mut(&key) {
                    it
                } else {
                    return Ok(MessageProcessing::Proceed)
                };

                file = sharer.file.try_clone()?;
                size = sharer.size.clone();
            }

            connection.send_file_non_blocking(file, size, id.clone())?;
        }
        ServerMessage::DeclineFileUpload { id, reason } => {
            let sharer = if let Some(it) = connection.remove_sharer(id.clone())? {
                it
            } else {
                return Ok(MessageProcessing::Proceed)
            };

            println!("(Server) Nah, wait with your #{}. {}", &sharer.name, &reason.clone())
        }
        ServerMessage::AgreeFileDownload { name, size, id } => {
            connection.promote_sharer(&name, size.clone(), id.clone())?;

            let response = ClientMessage::AgreeFileDownload {
                id: id.clone(),
            };

            connection.write_message(&response)?;
        }
        ServerMessage::DeclineFileDownload { name, reason } => {
            connection.remove_unpromoted_sharer(&name)?;
            println!("(Server) Nah, I won't give you {}. {}", &name.clone(), &reason.clone())
        }
        _ => {
            println!("{}", message);
        }
    }

    Ok(MessageProcessing::Proceed)
}

fn handle_server_messages(mut connection: impl ClientSession + 'static) -> Result<()> {
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
    connection: &mut impl ClientSession,
) -> Result<CommandProcessing> {
    match command {
        Command::Text { text } => {
            let message = ClientMessage::Text {
                text: text,
            };

            connection.write_message(&message)?;
        }
        Command::Rename { new_name } => {
            let message = ClientMessage::Rename {
                new_name: new_name,
            };

            connection.write_message(&message)?;
        }
        Command::UploadFile { name, path } => {
            let id = connection.free_id()?;

            let file = File::open(&path)?;
            let size = file.metadata()?.len() as usize;

            let request = ClientMessage::RequestFileUpload {
                name: name.clone(),
                size: size,
                id: id,
            };

            connection.prepare_sharer(&path, file, &name)?;
            connection.promote_sharer(&name, size, id)?;

            connection.write_message(&request)?;
        }
        Command::DownloadFile { name, path } => {
            connection.prepare_sharer(&path, File::create(&path)?, &name)?;

            let inner = ClientMessage::RequestFileDownload {
                name: name.clone(),
            };

            connection.write_message(&inner)?;
        }
        _ => {}
    }

    Ok(CommandProcessing::Proceed)
}

fn handle_user_command(
    connection: &mut Option<impl ClientSession>,
    reader: &mut Peekable<CharsReader>,
) -> Result<CommandProcessing> {
    match commands::parse(reader) {
        Command::End => {
            return Ok(CommandProcessing::Stop)
        }
        Command::Connect { address } => {
            let (
                reading_connection,
                writing_connection
            ) = build_client_connection(
                TcpStream::connect(address)?
            )?;

            std::thread::spawn(|| {
                with_error_report(|| handle_server_messages(reading_connection))
            });

            return Ok(CommandProcessing::Connect(writing_connection))
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

    let mut connection: Option<ClientSessionData> = None;

    loop {
        let result = handle_user_command(&mut connection, &mut reader)?;

        if let CommandProcessing::Stop = &result {
            break
        } else if let CommandProcessing::Connect(it) = result {
            connection = Some(it);
        }
    }

    if let Some(it) = &mut connection {
        it.write_message(&ClientMessage::Leave)?;
    }

    Ok(())
}

fn handle_connection() -> Result<()> {
    handle_user_commands()
}

pub fn start() {
    with_error_report(handle_connection);
}
