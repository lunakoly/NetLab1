mod chars_reader;
mod connection;
mod commands;

use std::fs::{File};
use std::io::{BufRead};
use std::net::{TcpStream};
use std::time::{Duration};
use std::sync::mpsc::{channel, Sender};

use connection::{
    ArsonClientSession,
    ClientSession,
    build_connection,
};

use shared::{Result, with_error_report, is_would_block_error};

use shared::connection::messages::{
    CommonMessage,
    ClientMessage,
    ServerMessage,
};

use shared::communication::{
    WriteMessage,
    explain_common_error,
    MessageProcessing,
};

use shared::connection::helpers::{
    process_sending_sharers,
};

use chars_reader::{IntoCharsReader};
use commands::{Command, CommandProcessing};

fn handle_server_chunk(
    connection: &mut (impl ClientSession + 'static),
    data: &[u8],
    id: usize,
) -> Result<MessageProcessing> {
    let done = connection.accept_chunk(data, id)?;

    if !done {
        return Ok(MessageProcessing::Proceed);
    }

    let sharer = if let Some(that) = connection.remove_sharer(id)? {
        that
    } else {
        return Ok(MessageProcessing::Proceed)
    };

    println!("(Console) Downloaded {} and saved to {}", &sharer.name, &sharer.path);
    Ok(MessageProcessing::Proceed)
}

fn handle_server_common_message(
    connection: &mut (impl ClientSession + 'static),
    message: &CommonMessage,
) -> Result<MessageProcessing> {
    match message {
        CommonMessage::Chunk { data, id } => {
            handle_server_chunk(connection, &data, id.clone())
        }
    }
}

fn handle_server_agree_file_upload(
    connection: &mut (impl ClientSession + 'static),
    id: usize,
) -> Result<MessageProcessing> {
    let sharer = if let Some(it) = connection.remove_sharer(id)? {
        it
    } else {
        return Ok(MessageProcessing::Proceed)
    };

    connection.enqueu_sending_sharer(sharer)?;
    Ok(MessageProcessing::Proceed)
}

fn handle_server_decline_file_upload(
    connection: &mut (impl ClientSession + 'static),
    id: usize,
    reason: &str,
) -> Result<MessageProcessing> {
    let sharer = if let Some(it) = connection.remove_sharer(id)? {
        it
    } else {
        return Ok(MessageProcessing::Proceed)
    };

    println!("(Server) Nah, wait with your #{}. {}", &sharer.name, &reason.clone());
    Ok(MessageProcessing::Proceed)
}

fn handle_server_agree_file_download(
    connection: &mut (impl ClientSession + 'static),
    name: &str,
    size: usize,
    id: usize,
) -> Result<MessageProcessing> {
    connection.promote_sharer(&name, size, id)?;

    let response = ClientMessage::AgreeFileDownload {
        id: id,
    };

    connection.write_message(&response)?;
    Ok(MessageProcessing::Proceed)
}

fn handle_server_decline_file_download(
    connection: &mut (impl ClientSession + 'static),
    name: &str,
    reason: &str,
) -> Result<MessageProcessing> {
    connection.remove_unpromoted_sharer(&name)?;
    println!("(Server) Nah, I won't give you {}. {}", &name, &reason);
    Ok(MessageProcessing::Proceed)
}

fn handle_server_message(
    connection: &mut (impl ClientSession + 'static),
    message: &ServerMessage,
) -> Result<MessageProcessing> {
    match message {
        ServerMessage::Common { common } => {
            handle_server_common_message(connection, &common)
        }
        ServerMessage::AgreeFileUpload { id } => {
            handle_server_agree_file_upload(connection, id.clone())
        }
        ServerMessage::DeclineFileUpload { id, reason } => {
            handle_server_decline_file_upload(connection, id.clone(), &reason)
        }
        ServerMessage::AgreeFileDownload { name, size, id } => {
            handle_server_agree_file_download(connection, &name, size.clone(), id.clone())
        }
        ServerMessage::DeclineFileDownload { name, reason } => {
            handle_server_decline_file_download(connection, &name, &reason)
        }
        _ => {
            println!("{}", message);
            Ok(MessageProcessing::Proceed)
        }
    }
}

fn read_and_handle_server_message(
    connection: &mut (impl ClientSession + 'static)
) -> Result<MessageProcessing> {
    let message = match connection.read_message() {
        Ok(it) => it,
        Err(error) => {
            if is_would_block_error(&error) {
                return Ok(MessageProcessing::ProceedButWaiting)
            }

            let explaination = explain_common_error(&error);
            println!("(Server) Error > {}", &explaination);
            return Ok(MessageProcessing::Stop)
        }
    };

    handle_server_message(connection, &message)
}

fn perform_text(
    connection: &mut impl ClientSession,
    text: &str,
) -> Result<CommandProcessing> {
    let message = ClientMessage::Text {
        text: text.to_owned(),
    };

    connection.write_message(&message)?;
    Ok(CommandProcessing::Proceed)
}

fn perform_rename(
    connection: &mut impl ClientSession,
    new_name: &str,
) -> Result<CommandProcessing> {
    let message = ClientMessage::Rename {
        new_name: new_name.to_owned(),
    };

    connection.write_message(&message)?;
    Ok(CommandProcessing::Proceed)
}

fn perform_upload_file(
    connection: &mut impl ClientSession,
    name: &str,
    path: &str,
) -> Result<CommandProcessing> {
    let id = connection.free_id()?;

    let file = File::open(path)?;
    let size = file.metadata()?.len() as usize;

    let request = ClientMessage::RequestFileUpload {
        name: name.to_owned(),
        size: size,
        id: id,
    };

    connection.prepare_sharer(path, file, name)?;
    connection.promote_sharer(name, size, id)?;

    connection.write_message(&request)?;
    Ok(CommandProcessing::Proceed)
}

fn perform_download_file(
    connection: &mut impl ClientSession,
    name: &str,
    path: &str,
) -> Result<CommandProcessing> {
    connection.prepare_sharer(path, File::create(path)?, name)?;

    let inner = ClientMessage::RequestFileDownload {
        name: name.to_owned(),
    };

    connection.write_message(&inner)?;
    Ok(CommandProcessing::Proceed)
}

fn match_user_command_with_connection(
    command: &Command,
    connection: &mut impl ClientSession,
) -> Result<CommandProcessing> {
    match command {
        Command::Text { text } => {
            perform_text(connection, &text)
        }
        Command::Rename { new_name } => {
            perform_rename(connection, &new_name)
        }
        Command::UploadFile { name, path } => {
            perform_upload_file(connection, &name, &path)
        }
        Command::DownloadFile { name, path } => {
            perform_download_file(connection, &name, &path)
        }
        _ => {
            Ok(CommandProcessing::Proceed)
        }
    }
}

fn handle_user_command(
    command: &Command,
    connection: &mut Option<impl ClientSession>,
) -> Result<CommandProcessing> {
    match command {
        Command::End => {
            return Ok(CommandProcessing::Stop)
        }
        Command::Connect { address } => {
            let (
                _,
                writing_connection
            ) = build_connection(
                TcpStream::connect(address)?
            )?;

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

fn read_user_command(
    send_command: Sender<Command>,
) -> Result<()> {
    let stdin = std::io::stdin();
    let lock: &mut dyn BufRead = &mut stdin.lock();
    let mut reader = lock.chars().peekable();

    loop {
        let command = commands::parse(&mut reader);
        send_command.send(command)?;
    }
}

const WAITING_DELAY_MILLIS: u64 = 16;

fn handle_connection() -> Result<()> {
    let mut connection: Option<ArsonClientSession> = None;

    let (
        send_command,
        read_command,
    ) = channel::<Command>();

    std::thread::spawn(|| {
        with_error_report(|| read_user_command(send_command))
    });

    loop {
        let mut did_something = false;

        if let Ok(command) = read_command.try_recv() {
            did_something = true;
            let result = handle_user_command(&command, &mut connection)?;

            if let CommandProcessing::Stop = &result {
                break
            } else if let CommandProcessing::Connect(it) = result {
                connection = Some(it);
            }
        }

        if let Some(the_connection) = &mut connection {
            let result = read_and_handle_server_message(the_connection)?;

            if let MessageProcessing::Stop = &result {
                connection = None;
                break
            }

            did_something |= !matches!(&result, MessageProcessing::ProceedButWaiting);
            did_something |= process_sending_sharers(the_connection)?;
        }

        if !did_something {
            std::thread::sleep(Duration::from_millis(WAITING_DELAY_MILLIS));
        }
    }

    if let Some(it) = &mut connection {
        it.write_message(&ClientMessage::Leave)?;
    }

    Ok(())
}

pub fn start() {
    with_error_report(handle_connection);
}
