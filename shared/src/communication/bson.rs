use std::io::{Read, Write};

use crate::{ErrorKind, Result};
use crate::communication::{ReadMessage, WriteMessage};

use crate::helpers::capped_reader::{
    IntoCappedReader,
    CappedReader,
    CappedRead,
};

use bson::Document;

pub fn from_reader_with_checked_errors<R: Read>(
    mut reader: R
) -> Result<Document> {
    match Document::from_reader(&mut reader) {
        Ok(it) => Ok(it),
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
                // 'failed to fill whole buffer'
                std::io::ErrorKind::UnexpectedEof |
                // the other side disconnects before sending
                // a single message
                std::io::ErrorKind::ConnectionReset => {
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

pub struct BsonReader<R> {
    stream: CappedReader<R>,
}

impl<R: Read> BsonReader<R> {
    pub fn new(reader: R, cap: usize) -> BsonReader<R> {
        BsonReader {
            stream: reader.to_capped(cap),
        }
    }
}

impl<R: Read> ReadMessage<Document> for BsonReader<R> {
    fn read_message(&mut self) -> Result<Document> {
        let result = from_reader_with_checked_errors(&mut self.stream);

        if let Ok(..) = result {
            self.stream.clear();
        }

        result
    }
}

pub struct BsonScanner<R> {
    stream: CappedReader<R>,
    buffer: Vec<u8>,
}

impl<R: Read> BsonScanner<R> {
    pub fn new(reader: R, cap: usize) -> BsonScanner<R> {
        BsonScanner {
            stream: reader.to_capped(cap),
            buffer: vec![],
        }
    }

    fn try_fetch_size(&self) -> Option<usize> {
        // See:
        // https://github.com/mongodb/bson-rust/blob/master/src/de/mod.rs#L145
        if self.buffer.len() < 4 {
            return None;
        }

        let mut size_field = [0u8; 4];

        for it in 0..4 {
            size_field[it] = self.buffer[it];
        }

        let size = i32::from_le_bytes(size_field) as usize;
        return Some(size);
    }

    fn parse(&mut self) -> Result<Document> {
        if let Some(size) = self.try_fetch_size() {
            if self.buffer.len() >= size {
                let result = from_reader_with_checked_errors(&mut self.buffer.as_slice());

                if let Ok(..) = result {
                    self.stream.clear();
                    self.buffer.drain(..size);
                }

                return result;
            }
        }

        Err(std::io::Error::from(std::io::ErrorKind::WouldBlock).into())
    }
}

impl<R: Read> ReadMessage<Document> for BsonScanner<R> {
    fn read_message(&mut self) -> Result<Document> {
        let mut new_data = vec![0u8; self.stream.space_left()];

        let count = match self.stream.read(&mut new_data) {
            Ok(count) => count,
            Err(error) => match error.kind() {
                // We might be unable to read something
                // new, but we might've read multiple
                // messages before, and now we need
                // to return them one by one from
                // the inner buffer
                std::io::ErrorKind::WouldBlock => {
                    0
                }
                // the other side disconnects before sending
                // a single message
                std::io::ErrorKind::ConnectionReset => {
                    return Err(ErrorKind::NothingToRead.into())
                }
                _ => {
                    return Err(error.into())
                }
            }
        };

        self.buffer.extend(&new_data[..count]);
        self.parse()
    }
}

pub struct BsonWriter<W> {
    stream: W,
}

impl<W> BsonWriter<W> {
    pub fn new(stream: W) -> BsonWriter<W> {
        BsonWriter {
            stream: stream,
        }
    }
}

impl<W: Write> WriteMessage<Document> for BsonWriter<W> {
    fn write_message(&mut self, message: &Document) -> Result<()> {
        // Idk, but &mut [0u8; N] doesn't work here,
        // it simply stays filled with 0
        let mut buffer = vec![];
        message.to_writer(&mut buffer)?;
        self.stream.write_all(&buffer)?;
        self.stream.flush()?;
        Ok(())
    }
}
