use std::net::{TcpListener, TcpStream};

use shared::{Result, with_error_report};

use shared::helpers::NamesMap;

use shared::communication::{
    ReadMessage,
    WriteMessage,
    try_explain_common_error,
};

use shared::communication::xxson::{
    ServerSideConnection,
    ServerMessage,
    VisualizeClientMessage,
    Connection,
};

use std::sync::{Arc, RwLock};

use std::collections::HashMap;

fn handle_input(connection: &mut ServerSideConnection, names: NamesMap) -> Result<()> {
    match connection.reader.read() {
        Ok(value) => {
            value.visualize(names, connection)?;

            let response = ServerMessage::Text {
                name: "Server".to_owned(),
                text: "Hi, got it.".to_owned(),
            };

            connection.writer.write(&response)?;

            Ok(())
        }
        Err(error) => {
            let prefix = connection.get_personality_prefix(names)?;

            if try_explain_common_error(&error, &prefix) {
                println!("{}Error > {}", &prefix, error);
            }

            Err(error)
        }
    }
}

fn handle_client(stream: TcpStream, names: NamesMap) -> Result<()> {
    let mut connection = ServerSideConnection::new(stream)?;

    loop {
        if let Err(..) = handle_input(&mut connection, names.clone()) {
            break
        }
    }

    Ok(())
}

fn setup_names_mapping() -> NamesMap {
    let mut names = HashMap::new();

    names.insert(
        "127.0.0.1".to_owned(),
        "Server".to_owned(),
    );

    Arc::new(RwLock::new(names))
}

fn handle_connection() -> Result<()> {
    let names = setup_names_mapping();

    let listener = TcpListener::bind("127.0.0.1:6969")?;

    for incomming in listener.incoming() {
        let stream = incomming?;
        let new_names = names.clone();
        std::thread::spawn(|| handle_client(stream, new_names));
    }

    Ok(())
}

pub fn start() {
    match with_error_report(handle_connection) {
        Ok(_) => println!("Good."),
        Err(_) => println!("Bad."),
    }
}
