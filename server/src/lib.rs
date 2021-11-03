mod connection;

use std::net::{TcpListener, TcpStream};
use std::collections::{HashMap};
use std::time::{Duration};
use std::path::{Path};
use std::fs::{File};

use shared::shared::{IntoShared, Shared};
use shared::communication::{DEFAULT_PORT};
use shared::{Result, with_error_report, ErrorKind, is_would_block_error};

use shared::communication::{
    explain_common_error,
    MessageProcessing,
};

use connection::{
    ArsonServerSession,
    ServerSession,
    NamesMap,
    Clients,
    build_connection,
    RenameResult,
};

use shared::connection::messages::{
    CommonMessage,
    ServerMessage,
    ClientMessage,
};

use shared::connection::messages::{
    MAXIMUM_TEXT_SIZE,
    MAXIMUM_NAME_SIZE,
    MAXIMUM_FILE_NAME_SIZE,
};

use shared::connection::helpers::{
    process_sending_sharers,
};

fn broadcast_interupt(
    connection: &mut impl ServerSession
) -> Result<MessageProcessing> {
    let time = chrono::Utc::now();
    let name = connection.name()?;

    let response = ServerMessage::Interrupt {
        name: name,
        time: time.into()
    };

    connection.broadcast(&response)?;
    Ok(MessageProcessing::Stop)
}

fn handle_upper_bound_violation(
    connection: &mut impl ServerSession,
    bounded_field_name: &str,
) -> Result<MessageProcessing> {
    let time = chrono::Utc::now();
    let name = connection.name()?;

    println!("<{}> Error > {} tried to sabotage the party by violating the {} size bound. Terminated.", &time, &name, bounded_field_name);

    connection.remove_from_clients()?;
    return broadcast_interupt(connection);
}

fn handle_client_chunk(
    connection: &mut (impl ServerSession + 'static),
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

    let response = ServerMessage::NewFile {
        name: sharer.name,
    };

    connection.broadcast(&response)?;
    Ok(MessageProcessing::Proceed)
}

fn handle_client_common_message(
    connection: &mut (impl ServerSession + 'static),
    message: &CommonMessage,
) -> Result<MessageProcessing> {
    match message {
        CommonMessage::Chunk { data, id } => {
            handle_client_chunk(connection, data, id.clone())
        }
    }
}

fn handle_client_text(
    connection: &mut (impl ServerSession + 'static),
    text: &str,
) -> Result<MessageProcessing> {
    let time = chrono::Utc::now();
    let name = connection.name()?;

    if text.len() > MAXIMUM_TEXT_SIZE {
        return handle_upper_bound_violation(connection, "text");
    }

    println!("<{}> Message > {} > {}", &time, &name, text);

    let response = ServerMessage::Text {
        name: name,
        text: text.to_owned(),
        time: time.into()
    };

    connection.broadcast(&response)?;
    Ok(MessageProcessing::Proceed)
}

fn handle_client_leave(
    connection: &mut (impl ServerSession + 'static),
) -> Result<MessageProcessing> {
    let time = chrono::Utc::now();
    let name = connection.name()?;

    connection.remove_from_clients()?;
    println!("<{}> User Leaves > {}", &time, &name);

    let response = ServerMessage::UserLeaves {
        name: name,
        time: time.into()
    };

    connection.broadcast(&response)?;
    Ok(MessageProcessing::Stop)
}

fn handle_client_rename(
    connection: &mut (impl ServerSession + 'static),
    new_name: &str,
) -> Result<MessageProcessing> {
    if new_name.len() > MAXIMUM_NAME_SIZE {
        return handle_upper_bound_violation(connection, "name");
    }

    match connection.rename(new_name)? {
        RenameResult::Success { old_name, new_name } => {
            let message = ServerMessage::UserRenamed { old_name, new_name };
            connection.write_message(&message)?;
        }
        RenameResult::Failure { reason } => {
            let message = ServerMessage::Support { text: reason };
            connection.write_message(&message)?;
        }
    }

    Ok(MessageProcessing::Proceed)
}

fn handle_client_request_file_upload(
    connection: &mut (impl ServerSession + 'static),
    name: &str,
    size: usize,
    id: usize,
) -> Result<MessageProcessing> {
    if name.len() > MAXIMUM_FILE_NAME_SIZE {
        return handle_upper_bound_violation(connection, "file name");
    }

    let response = if Path::new(name).exists() {
        ServerMessage::DeclineFileUpload {
            id: id,
            reason: "There's already a file with such a name".to_owned(),
        }
    } else {
        connection.prepare_sharer(name, File::create(name)?, name)?;
        connection.promote_sharer(name, size, id)?;

        ServerMessage::AgreeFileUpload {
            id: id,
        }
    };

    connection.write_message(&response)?;
    Ok(MessageProcessing::Proceed)
}

fn handle_client_request_file_download(
    connection: &mut (impl ServerSession + 'static),
    name: &str,
) -> Result<MessageProcessing> {
    if name.len() > MAXIMUM_FILE_NAME_SIZE {
        return handle_upper_bound_violation(connection, "file name");
    }

    let response = if !Path::new(name).exists() {
        ServerMessage::DeclineFileDownload {
            name: name.to_owned(),
            reason: "There's no such a file".to_owned(),
        }
    } else {
        let id = connection.free_id()?;

        let file = File::open(name)?;
        let size = file.metadata()?.len() as usize;

        connection.prepare_sharer(name, file, name)?;
        connection.promote_sharer(name, size, id)?;

        ServerMessage::AgreeFileDownload {
            name: name.to_owned(),
            id: id,
            size: size,
        }
    };

    connection.write_message(&response)?;
    Ok(MessageProcessing::Proceed)
}

fn handle_client_agree_file_download(
    connection: &mut (impl ServerSession + 'static),
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

fn handle_client_decline_file_download(
    connection: &mut (impl ServerSession + 'static),
    id: usize,
) -> Result<MessageProcessing> {
    // Well, they asked for the file, but
    // now they say they can't accept the size.
    connection.remove_sharer(id)?;
    Ok(MessageProcessing::Proceed)
}

fn handle_client_message(
    connection: &mut (impl ServerSession + 'static),
    message: &ClientMessage,
) -> Result<MessageProcessing> {
    match message {
        ClientMessage::Common { common } => {
            handle_client_common_message(connection, &common)
        }
        ClientMessage::Text { text } => {
            handle_client_text(connection, &text)
        }
        ClientMessage::Leave => {
            handle_client_leave(connection)
        }
        ClientMessage::Rename { new_name } => {
            handle_client_rename(connection, &new_name)
        }
        ClientMessage::RequestFileUpload { name, size, id } => {
            handle_client_request_file_upload(connection, &name, size.clone(), id.clone())
        }
        ClientMessage::RequestFileDownload { name } => {
            handle_client_request_file_download(connection, &name)
        }
        ClientMessage::AgreeFileDownload { id } => {
            handle_client_agree_file_download(connection, id.clone())
        }
        ClientMessage::DeclineFileDownload { id } => {
            handle_client_decline_file_download(connection, id.clone())
        }
    }
}

fn read_and_handle_client_message(
    connection: &mut (impl ServerSession + 'static)
) -> Result<MessageProcessing> {
    let time = chrono::Utc::now();
    let name = connection.name()?;

    let message = match connection.read_message() {
        Ok(it) => it,
        Err(error) => {
            if is_would_block_error(&error) {
                return Ok(MessageProcessing::ProceedButWaiting)
            }

            let explaination = explain_common_error(&error);
            println!("<{}> Error > {} > {}", &time, &name, &explaination);

            if let ErrorKind::NothingToRead = error.kind {
                return broadcast_interupt(connection);
            }

            return Err(error)
        }
    };

    handle_client_message(connection, &message)
}

fn setup_names_mapping() -> NamesMap {
    let mut names = HashMap::new();

    names.insert(
        "127.0.0.1:6969".to_owned(),
        "Server".to_owned(),
    );

    names.to_shared()
}

fn greet_user(
    writing_connection: &mut impl ServerSession,
) -> Result<String> {
    let time = chrono::Utc::now();
    let name = writing_connection.name()?;

    println!("<{}> New User > {}", &time, &name);

    let broadcast_greeting = ServerMessage::NewUser {
        name: name,
        time: time.into()
    };

    writing_connection.broadcast(&broadcast_greeting)?;

    let personal_greeting = ServerMessage::Support {
        text: "Welcome to the club, mate".to_owned(),
    };

    writing_connection.write_message(&personal_greeting)?;
    Ok(writing_connection.remote_address()?.to_string())
}

fn handle_client(
    stream: TcpStream,
    names: NamesMap,
    clients: Clients,
    connections: &mut Vec<Shared<ArsonServerSession>>,
) -> Result<()> {
    let (
        mut reading_connection,
        _
    ) = build_connection(
        stream,
        names,
        clients.clone(),
    )?;

    let address = greet_user(&mut reading_connection)?;
    let shared = reading_connection.to_shared();

    clients.insert(address, shared.clone())?;
    connections.push(shared);

    Ok(())
}

fn process_connections(
    connections: &mut Vec<Shared<ArsonServerSession>>,
) -> Result<bool> {
    if connections.len() == 0 {
        return Ok(false)
    }

    let mut to_be_removed = vec![];
    let mut did_something = false;

    for (index, it) in connections.iter_mut().enumerate() {
        let result = read_and_handle_client_message(it)?;

        if let MessageProcessing::Stop = &result {
            to_be_removed.push(index);
            break
        }

        did_something |= !matches!(&result, MessageProcessing::ProceedButWaiting);
        did_something |= process_sending_sharers(it)?;
    }

    let mut removed_count = 0usize;

    for it in to_be_removed {
        connections.remove(it - removed_count);
        removed_count += 1;
    }

    Ok(did_something)
}

const WAITING_DELAY_MILLIS: u64 = 16;

fn handle_connection() -> Result<()> {
    let names = setup_names_mapping();
    let clients = HashMap::new().to_shared();

    let listener = TcpListener::bind(format!("0.0.0.0:{}", DEFAULT_PORT))?;
    listener.set_nonblocking(true)?;

    let mut connections = vec![];

    for incomming in listener.incoming() {
        let mut did_something = false;

        match incomming {
            Ok(it) => {
                handle_client(it, names.clone(), clients.clone(), &mut connections)?;
            }
            Err(error) => {
                if !matches!(error.kind(), std::io::ErrorKind::WouldBlock) {
                    return Err(error.into())
                }
            }
        }

        did_something |= process_connections(&mut connections)?;

        if !did_something {
            std::thread::sleep(Duration::from_millis(WAITING_DELAY_MILLIS));
        }
    }

    Ok(())
}

pub fn start() {
    with_error_report(handle_connection);
}
