use std::io::{Read, Write};

use crate::{ErrorKind, Result};
use crate::communication::bson::{BsonReader, BsonScanner, BsonWriter};

use crate::communication::{
    ReadMessage,
    WriteMessage,
};

use bson::doc;

pub struct ArsonReader<R> {
    backend: BsonReader<R>,
}

impl<R: Read> ArsonReader<R> {
    pub fn new(reader: R, cap: usize) -> ArsonReader<R> {
        ArsonReader {
            backend: BsonReader::new(reader, cap),
        }
    }
}

impl<R, M> ReadMessage<M> for ArsonReader<R>
where
    R: Read,
    M: for<'de> serde::Deserialize<'de>,
{
    fn read_message(&mut self) -> Result<M> {
        let it = self.backend.read_message()?;
        let message: M = bson::from_bson(it.into())?;
        Ok(message)
    }
}

pub struct ArsonScanner<R> {
    backend: BsonScanner<R>,
}

impl<R: Read> ArsonScanner<R> {
    pub fn new(reader: R, cap: usize) -> ArsonScanner<R> {
        ArsonScanner {
            backend: BsonScanner::new(reader, cap),
        }
    }
}

impl<R, M> ReadMessage<M> for ArsonScanner<R>
where
    R: Read,
    M: for<'de> serde::Deserialize<'de>,
{
    fn read_message(&mut self) -> Result<M> {
        let it = self.backend.read_message()?;
        let message: M = bson::from_bson(it.into())?;
        Ok(message)
    }
}

pub struct ArsonWriter<W> {
    backend: BsonWriter<W>,
}

impl<W: Write> ArsonWriter<W> {
    pub fn new(stream: W) -> ArsonWriter<W> {
        ArsonWriter {
            backend: BsonWriter::new(stream),
        }
    }
}

impl<W, M> WriteMessage<M> for ArsonWriter<W>
where
    W: Write,
    M: serde::Serialize,
{
    fn write_message(&mut self, message: &M) -> Result<()> {
        let serialized = bson::to_bson(message)?;

        if let Some(it) = serialized.as_document() {
            self.backend.write_message(it)
        } else if let Some(it) = serialized.as_str() {
            let wrapper = doc! {
                it: {}
            };

            self.backend.write_message(&wrapper)
        } else {
            Err(ErrorKind::NothingToRead.into())
        }
    }
}
