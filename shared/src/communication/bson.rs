use crate::{ErrorKind, Result};
use crate::communication::{ReadMessage, WriteMessage};

use crate::capped_reader::{
    CappedReader,
    CappedRead,
};

use std::io::prelude::Write;
use std::io::Read;

use std::net::TcpStream;

use bson::Document;

pub struct BsonReader<R> {
    stream: CappedReader<R>,
}

impl<R: Read> BsonReader<R> {
    pub fn new(capped_reader: CappedReader<R>) -> BsonReader<R> {
        BsonReader {
            stream: capped_reader,
        }
    }
}

impl<'a, R: Read> ReadMessage<Document> for BsonReader<R> {
    fn read(&mut self) -> Result<Document> {
        match Document::from_reader(&mut self.stream) {
            Ok(it) => {
                self.stream.clear();
                Ok(it)
            }
            // All this error unwrapping is to
            // ensure the same behavior as for
            // JsonReader
            Err(error) => match error {
                bson::de::Error::EndOfStream => {
                    let kind = ErrorKind::Io {
                        source: std::io::ErrorKind::InvalidData.into()
                    };
                    Err(kind.into())
                }
                bson::de::Error::Io(ref io_error) => match io_error.kind() {
                    std::io::ErrorKind::UnexpectedEof => {
                        Err(ErrorKind::NothingToRead.into())
                    }
                    _ => {
                        Err(error.into())
                    }
                }
                _ => {
                    Err(error.into())
                }
            }
        }
    }
}

pub struct BsonWriter {
    pub stream: TcpStream,
}

impl BsonWriter {
    pub fn new(stream: TcpStream) -> BsonWriter {
        BsonWriter {
            stream: stream,
        }
    }
}

impl WriteMessage<&Document> for BsonWriter {
    fn write(&mut self, message: &Document) -> Result<()> {
        // Idk, but &mut [0u8; N] doesn't work here,
        // it simply stays filled with 0
        let mut buffer = vec![];
        message.to_writer(&mut buffer)?;
        self.stream.write(&buffer)?;
        self.stream.flush()?;
        Ok(())
    }
}
