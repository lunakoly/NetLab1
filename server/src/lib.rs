use std::thread;

use std::net::{TcpListener, TcpStream};
use std::collections::{HashMap};
use std::sync::{Arc, RwLock};
use std::cell::{RefCell};

use shared::communication::{DEFAULT_PORT};
use shared::helpers::{TcpSplit, with_refcell};
use shared::{Result, with_error_report, ErrorKind};

use shared::communication::{
    explain_common_error,
    MessageProcessing,
};

use shared::communication::xxson::connection::{
    Connection,
    ServerConnection,
    ServerReadingConnection,
    ServerReadingContext,
    ServerWritingContext,
    NamesMap,
    Clients,
};

use shared::communication::xxson::{
    ServerMessage,
    ClientMessage,
    XXsonWriter,
};

fn handle_client_mesage<C>(connection: &mut C) -> Result<MessageProcessing>
where
    C: ServerReadingConnection,
{
    let time = chrono::Utc::now();
    let name = connection.get_name()?;

    let message = match connection.read() {
        Ok(it) => it,
        Err(error) => {
            let explaination = explain_common_error(&error);
            println!("<{}> Error > {} > {}", &time, &name, &explaination);

            if let ErrorKind::NothingToRead = error.kind {
                let response = ServerMessage::Interrupt {
                    name: name,
                    time: time.into()
                };

                connection.broadcast(&response)?;
            }

            return Ok(MessageProcessing::Stop)
        }
    };

    match message {
        ClientMessage::Text { text } => {
            println!("<{}> Message > {} > {}", &time, &name, &text);

            let response = ServerMessage::Text {
                name: name,
                text: text,
                time: time.into()
            };

            connection.broadcast(&response)?;
        }
        ClientMessage::Leave => {
            // Early user removal, prevents
            // broadcasting to the broken connection.
            connection.remove_current_writer()?;

            println!("<{}> User Leaves > {}", &time, &name);

            let response = ServerMessage::UserLeaves {
                name: name,
                time: time.into()
            };

            connection.broadcast(&response)?;
            return Ok(MessageProcessing::Stop)
        }
        ClientMessage::Rename { new_name } => {
            connection.rename(&new_name)?;
        }
    }

    Ok(MessageProcessing::Proceed)
}

fn handle_client_messages<C>(mut connection: C) -> Result<()>
where
    C: ServerReadingConnection,
{
    loop {
        let result = handle_client_mesage(&mut connection)?;

        if let MessageProcessing::Stop = &result {
            connection.remove_current_writer()?;
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
    stream: &RefCell<TcpStream>,
    names: NamesMap,
    clients: Clients,
) -> Result<String> {
    let mut writing_connection = ServerWritingContext::new(
        stream,
        names.clone(),
        clients.clone(),
    );

    let time = chrono::Utc::now();
    let name = writing_connection.get_name()?;

    println!("<{}> New User > {}", &time, &name);

    let greeting = ServerMessage::NewUser {
        name: name,
        time: time.into()
    };

    writing_connection.broadcast(&greeting)?;
    Ok(writing_connection.get_remote_address()?.to_string())
}

fn handle_connection() -> Result<()> {
    let names = setup_names_mapping();
    let clients = Arc::new(RwLock::new(HashMap::new()));
    let listener = TcpListener::bind(format!("127.0.0.1:{}", DEFAULT_PORT))?;

    for incomming in listener.incoming() {
        let (writing_stream, reading_stream) = incomming?.split()?;

        // Temporary create the full ServerWritingContext
        // infrastructure, but then break it all back down
        // to the TcpStream, so that we could create
        // a minimal writer that would take ownership over
        // this stream. ServerWritingContext can only work
        // with references.

        let (writing_stream, address) = with_refcell(
            writing_stream,
            |wrapped| greet_user(wrapped, names.clone(), clients.clone())
        )?;

        let new_names = names.clone();
        let new_clients = clients.clone();

        thread::spawn(|| {
            let wrapped = RefCell::new(reading_stream);

            let reading_connection = ServerReadingContext::new(
                &wrapped,
                new_names,
                new_clients
            );

            with_error_report(|| handle_client_messages(reading_connection))
        });

        let writer: XXsonWriter<TcpStream, ServerMessage> = XXsonWriter::new(writing_stream);
        clients.write()?.insert(address, writer);
    }

    Ok(())
}

pub fn start() {
    match with_error_report(handle_connection) {
        Ok(_) => println!("Good."),
        Err(_) => println!("Bad."),
    }
}
