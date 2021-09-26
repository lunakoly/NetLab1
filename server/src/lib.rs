use std::net::{TcpListener, TcpStream};

use std::io::Write;

use shared::{Result, Error, with_error_report};

use shared::communication::{
    ReadMessage,
    WriteMessage,
    try_explain_common_error,
};

use shared::communication::xxson::{
    ServerSideConnection,
    ServerMessage,
    Visualize,
    Connection,
};

pub fn handle_error(
    connection: &mut ServerSideConnection,
    error: &Error,
) -> Result<()> {
    println!("Error > {}", error);

    let response = "HTTP/1.1 404 OK\r\n\r\n";

    connection.get_raw_tcp_stream().write(response.as_bytes())?;
    connection.get_raw_tcp_stream().flush()?;

    Ok(())
}

pub fn handle_input(connection: &mut ServerSideConnection) -> Result<()> {
    match connection.reader.read() {
        Ok(value) => {
            value.visualize(connection)?;

            let response = ServerMessage::Text {
                name: "Server".to_owned(),
                text: "Hi, got it.".to_owned(),
            };

            connection.writer.write(&response)?;

            Ok(())
        }
        Err(error) => {
            if try_explain_common_error(&error) {
                handle_error(connection, &error)?;
            }

            Err(error)
        }
    }
}

pub fn handle_client(stream: TcpStream) -> Result<()> {
    let mut connection = ServerSideConnection::new(stream)?;

    loop {
        if let Err(..) = handle_input(&mut connection) {
            break
        }
    }

    Ok(())
}

pub fn handle_connection() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6969")?;

    for incomming in listener.incoming() {
        let stream = incomming?;
        std::thread::spawn(|| handle_client(stream));
    }

    Ok(())
}

pub fn start() {
    match with_error_report(handle_connection) {
        Ok(_) => println!("Good."),
        Err(_) => println!("Bad."),
    }
}
