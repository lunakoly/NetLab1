use std::thread;

use std::net::{TcpListener};
use std::collections::{HashMap};
use std::sync::{Arc, RwLock};

use shared::communication::{DEFAULT_PORT};
use shared::{Result, with_error_report, ErrorKind};

use shared::communication::{
    explain_common_error,
    MessageProcessing,
};

use shared::communication::xxson::connection::{
    ServerConnection,
    NamesMap,
    build_server_connection,
};

use shared::communication::xxson::{
    ServerMessage,
    ClientMessage,
    MAXIMUM_TEXT_SIZE,
    MAXIMUM_NAME_SIZE,
};

fn broadcast_interupt(
    connection: &mut impl ServerConnection
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
    connection: &mut impl ServerConnection
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

            connection.rename(&new_name)?;
        }
    }

    Ok(MessageProcessing::Proceed)
}

fn handle_client_messages(
    mut connection: impl ServerConnection
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

    Arc::new(RwLock::new(names))
}

fn greet_user(
    writing_connection: &mut impl ServerConnection,
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

fn handle_connection() -> Result<()> {
    let names = setup_names_mapping();
    let clients = Arc::new(RwLock::new(HashMap::new()));
    let listener = TcpListener::bind(format!("127.0.0.1:{}", DEFAULT_PORT))?;

    for incomming in listener.incoming() {
        let (
            reading_connection,
            mut writing_connection
        ) = build_server_connection(
            incomming?,
            names.clone(),
            clients.clone(),
        )?;

        let address = greet_user(&mut writing_connection)?;

        thread::spawn(|| {
            with_error_report(|| handle_client_messages(reading_connection))
        });

        clients.write()?.insert(address, writing_connection);
    }

    Ok(())
}

pub fn start() {
    match with_error_report(handle_connection) {
        Ok(_) => println!("Good."),
        Err(_) => println!("Bad."),
    }
}
