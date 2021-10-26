use std::io::{Read, Write};

use crate::{ErrorKind, Result};
use crate::helpers::capped_reader::{CappedReader, CappedRead};
use crate::communication::{ReadMessage, WriteMessage};

use serde_json::{Deserializer, Value};

pub struct JsonReader<R> {
    stream: CappedReader<R>,
}

impl<R> JsonReader<R> {
    pub fn new(capped_reader: CappedReader<R>) -> JsonReader<R> {
        JsonReader {
            stream: capped_reader,
        }
    }
}

impl<R: Read> ReadMessage<Value> for JsonReader<R> {
    fn read_message(&mut self) -> Result<Value> {
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
    stream: W,
}

impl<W> JsonWriter<W> {
    pub fn new(stream: W) -> JsonWriter<W> {
        JsonWriter {
            stream: stream,
        }
    }
}

impl<W: Write> WriteMessage<Value> for JsonWriter<W> {
    fn write_message(&mut self, message: &Value) -> Result<()> {
        self.stream.write(message.to_string().as_bytes())?;
        self.stream.flush()?;
        Ok(())
    }
}
