mod connection;

use std::thread;

use std::net::{TcpListener, TcpStream};
use std::collections::{HashMap};
use std::path::{Path};
use std::fs::{File};

use shared::shared::{Shared};
use shared::communication::{DEFAULT_PORT, SendFile};
use shared::{Result, with_error_report, ErrorKind};

use shared::communication::{
    explain_common_error,
    MessageProcessing,
};

use connection::{
    ServerSession,
    NamesMap,
    Clients,
    build_connection,
};

use shared::connection::messages::{
    CommonMessage,
    ServerMessage,
    ClientMessage,
};

use shared::connection::helpers::{
    MAXIMUM_TEXT_SIZE,
    MAXIMUM_NAME_SIZE,
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

fn handle_client_mesage(
    connection: &mut (impl ServerSession + 'static)
) -> Result<MessageProcessing> {
    let time = chrono::Utc::now();
    let name = connection.name()?;

    let message = match connection.read_message() {
        Ok(it) => it,
        Err(error) => {
            let explaination = explain_common_error(&error);
            println!("<{}> Error > {} > {}", &time, &name, &explaination);

            if let ErrorKind::NothingToRead = error.kind {
                return broadcast_interupt(connection);
            }

            return Ok(MessageProcessing::Stop)
        }
    };

    match message {
        ClientMessage::Common { common } => match common {
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

                let response = ServerMessage::NewFile {
                    name: sharer.name,
                };

                connection.broadcast(&response)?;
            }
        }
        ClientMessage::Text { text } => {
            if text.len() > MAXIMUM_TEXT_SIZE {
                connection.remove_from_clients()?;
                println!("<{}> Error > {} tried to sabotage the party by violating the text size bound. Terminated.", &time, &name);
                return broadcast_interupt(connection);
            }

            println!("<{}> Message > {} > {}", &time, &name, &text);

            let response = ServerMessage::Text {
                name: name,
                text: text,
                time: time.into()
            };

            connection.broadcast(&response)?;
        }
        ClientMessage::Leave => {
            connection.remove_from_clients()?;

            println!("<{}> User Leaves > {}", &time, &name);

            let response = ServerMessage::UserLeaves {
                name: name,
                time: time.into()
            };

            connection.broadcast(&response)?;
            return Ok(MessageProcessing::Stop)
        }
        ClientMessage::Rename { new_name } => {
            if new_name.len() > MAXIMUM_NAME_SIZE {
                connection.remove_from_clients()?;
                println!("<{}> Error > {} tried to sabotage the party by violating the name size bound. Terminated.", &time, &name);
                return broadcast_interupt(connection);
            }

            if let Some(it) = connection.rename(&new_name)? {
                connection.write_message(&it)?;
            };
        }
        ClientMessage::RequestFileUpload { name, size, id } => {
            let response = if Path::new(&name).exists() {
                ServerMessage::DeclineFileUpload {
                    id: id.clone(),
                    reason: "There's already a file with such a name".to_owned(),
                }
            } else {
                connection.prepare_sharer(&name, File::create(&name)?, &name)?;
                connection.promote_sharer(&name, size.clone(), id.clone())?;

                ServerMessage::AgreeFileUpload {
                    id: id.clone(),
                }
            };

            connection.write_message(&response)?;
        }
        ClientMessage::RequestFileDownload { name } => {
            let response = if !Path::new(&name).exists() {
                ServerMessage::DeclineFileDownload {
                    name: name.clone(),
                    reason: "There's no such a file".to_owned(),
                }
            } else {
                let id = connection.free_id()?;

                let file = File::open(&name)?;
                let size = file.metadata()?.len() as usize;

                connection.prepare_sharer(&name, file, &name)?;
                connection.promote_sharer(&name, size.clone(), id.clone())?;

                ServerMessage::AgreeFileDownload {
                    name: name.clone(),
                    id: id.clone(),
                    size: size,
                }
            };

            connection.write_message(&response)?;
        }
        ClientMessage::AgreeFileDownload { id } => {
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
        ClientMessage::DeclineFileDownload { id } => {
            // Well, they asked for the file, but
            // now they say they can't accept the size.
            connection.remove_sharer(id.clone())?;
        }
    }

    Ok(MessageProcessing::Proceed)
}

fn handle_client_messages(
    mut connection: impl ServerSession + 'static
) -> Result<()> {
    loop {
        let result = handle_client_mesage(&mut connection)?;

        if let MessageProcessing::Stop = &result {
            connection.remove_from_clients()?;
            break
        }
    }

    Ok(())
}

fn setup_names_mapping() -> NamesMap {
    let mut names = HashMap::new();

    names.insert(
        "127.0.0.1:6969".to_owned(),
        "Server".to_owned(),
    );

    Shared::new(names)
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
) -> Result<()> {
    let (
        reading_connection,
        mut writing_connection
    ) = build_connection(
        stream,
        names,
        clients.clone(),
    )?;

    let address = greet_user(&mut writing_connection)?;
    clients.insert(address, Shared::new(writing_connection))?;

    with_error_report(|| handle_client_messages(reading_connection));
    Ok(())
}

fn handle_connection() -> Result<()> {
    let names = setup_names_mapping();
    let clients = Shared::new(HashMap::new());
    let listener = TcpListener::bind(format!("127.0.0.1:{}", DEFAULT_PORT))?;

    for incomming in listener.incoming() {
        let the_names = names.clone();
        let the_clients = clients.clone();

        thread::spawn(|| {
            with_error_report(|| handle_client(incomming?, the_names, the_clients))
        });
    }

    Ok(())
}

pub fn start() {
    with_error_report(handle_connection);
}
