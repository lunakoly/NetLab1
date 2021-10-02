use crate::{ErrorKind, Result};
use crate::capped_reader::{CappedReader, CappedRead};
use crate::communication::{ReadMessage, WriteMessage};

use std::io::{Read, Write};

use serde_json::{Deserializer, Value};

pub struct JsonReader<R> {
    pub stream: CappedReader<R>,
}

impl<R> JsonReader<R> {
    pub fn new(capped_reader: CappedReader<R>) -> JsonReader<R> {
        JsonReader {
            stream: capped_reader,
        }
    }
}

impl<R: Read> ReadMessage<Value> for JsonReader<R> {
    fn read(&mut self) -> Result<Value> {
        let mut iterator = Deserializer::from_reader(&mut self.stream).into_iter::<Value>();

        match iterator.next() {
            Some(Ok(it)) => {
                self.stream.clear();
                Ok(it)
            }
            Some(Err(serde)) => {
                Err(serde.into())
            }
            _ => {
                Err(ErrorKind::NothingToRead.into())
            }
        }
    }
}

pub struct JsonWriter<W> {
    pub stream: W,
}

impl<W> JsonWriter<W> {
    pub fn new(stream: W) -> JsonWriter<W> {
        JsonWriter {
            stream: stream,
        }
    }
}

impl<W: Write> WriteMessage<Value> for JsonWriter<W> {
    fn write(&mut self, message: &Value) -> Result<()> {
        self.stream.write(message.to_string().as_bytes())?;
        self.stream.flush()?;
        Ok(())
    }
}
