use crate::Result;
use crate::capped_reader::IntoCappedReader;
use crate::communication::bson::{BsonReader, BsonWriter};

use std::net::TcpStream;

pub struct Connection {
    pub writer: BsonWriter,
    pub reader: BsonReader<TcpStream>,
}

pub fn prepare(stream: TcpStream) -> Result<Connection> {
    let connection = Connection {
        writer: BsonWriter::new(stream.try_clone()?),
        reader: BsonReader::new(stream.capped()),
    };

    Ok(connection)
}
