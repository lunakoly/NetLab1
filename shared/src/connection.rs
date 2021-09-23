use crate::Result;
use crate::capped_reader::IntoCappedReader;
use crate::communication::json::{JsonReader, JsonWriter};

use std::net::TcpStream;

pub struct Connection {
    pub writer: JsonWriter,
    pub reader: JsonReader<TcpStream>,
}

pub fn prepare(stream: TcpStream) -> Result<Connection> {
    let connection = Connection {
        writer: JsonWriter::new(stream.try_clone()?),
        reader: JsonReader::new(stream.capped()),
    };

    Ok(connection)
}
