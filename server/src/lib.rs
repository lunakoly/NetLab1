use std::net::{TcpListener, TcpStream};

use shared::{Result, with_error_report};

use shared::helpers::{NamesMap, SafeVec};

use shared::communication::{
    ReadMessage,
    WriteMessage,
    try_explain_common_error,
};

use shared::communication::xxson::{
    ServerSideConnection,
    ServerMessage,
    Connection,
    WritingKnot,
    XXsonReader,
    ClientMessage,
};

use std::sync::{Arc, RwLock};

use std::collections::HashMap;

use std::thread;

fn handle_input(
    reader: &mut XXsonReader<TcpStream, ClientMessage>,
    names: NamesMap,
    messages: SafeVec<ServerMessage>,
) -> Result<()> {
    let time = chrono::Utc::now();
    let name = reader.get_name(names)?;

    match reader.read() {
        Ok(value) => match value {
            ClientMessage::Text { text } => {
                println!("<{}> Message > {} > {}", &time, &name, &text);

                let response = ServerMessage::Text {
                    name: name,
                    text: text,
                    time: time.into()
                };

                messages.write()?.push(response);
                Ok(())
            }
        }
        Err(error) => {
            let explaination = match try_explain_common_error(&error) {
                Some(thing) => thing,
                None => format!("{}", error)
            };

            println!("<{}> Error > {} > {}", &time, &name, &explaination);
            Err(error)
        }
    }
}

fn handle_client_messages(
    mut reader: XXsonReader<TcpStream, ClientMessage>,
    names: NamesMap,
    clients: WritingKnot<TcpStream, ServerMessage>,
    messages: SafeVec<ServerMessage>,
) -> Result<()> {
    loop {
        if let Err(..) = handle_input(&mut reader, names.clone(), messages.clone()) {
            let mut the_clients = clients.write()?;
            let mut current: Option<usize> = None;

            for (index, it) in the_clients.iter().enumerate() {
                if it.get_remote_address()? == reader.get_remote_address()? {
                    current = Some(index);
                }
            }

            // In fact, the corresponding
            // writer is always present.
            if let Some(index) = current {
                the_clients.remove(index);
            }

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

fn send_broadcast(
    message: &ServerMessage,
    clients: WritingKnot<TcpStream, ServerMessage>,
) -> Result<()> {
    let mut the_clients = clients.write()?;

    for writer in the_clients.iter_mut() {
        writer.write(message)?;
    }

    Ok(())
}

fn handle_broadcast_queue(
    clients: WritingKnot<TcpStream, ServerMessage>,
    messages: SafeVec<ServerMessage>,
) -> Result<()> {
    loop {
        thread::sleep(std::time::Duration::from_millis(16));

        let mut the_messages = messages.write()?;

        if the_messages.is_empty() {
            continue
        }

        let message = the_messages.remove(0);
        send_broadcast(&message, clients.clone())?;
    }
}

fn handle_connection() -> Result<()> {
    let names = setup_names_mapping();

    let clients = Arc::new(RwLock::new(vec![]));
    let messages = Arc::new(RwLock::new(vec![]));

    let new_clients = clients.clone();
    let new_messages = messages.clone();

    thread::spawn(|| handle_broadcast_queue(new_clients, new_messages));

    let listener = TcpListener::bind("127.0.0.1:6969")?;

    for incomming in listener.incoming() {
        let connection = ServerSideConnection::new(incomming?)?;

        let new_names = names.clone();
        let new_clients = clients.clone();
        let new_messages = messages.clone();

        let reader = connection.reader;

        let time = chrono::Utc::now();
        let name = reader.get_name(names.clone())?;

        println!("<{}> New user > {}", &time, &name);

        let greeting = ServerMessage::NewUser {
            name: name,
            time: time.into()
        };

        send_broadcast(&greeting, clients.clone())?;

        thread::spawn(|| handle_client_messages(reader, new_names, new_clients, new_messages));

        clients.write()?.push(connection.writer);
    }

    Ok(())
}

pub fn start() {
    match with_error_report(handle_connection) {
        Ok(_) => println!("Good."),
        Err(_) => println!("Bad."),
    }
}
