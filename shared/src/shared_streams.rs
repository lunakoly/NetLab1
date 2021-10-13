use std::io::{Read, Write};
use std::sync::{Arc, RwLock};

pub struct SharedStream<R> {
   pub stream: Arc<RwLock<R>>,
}

impl<R> SharedStream<R> {
    pub fn new(stream: R) -> SharedStream<R> {
        SharedStream {
            stream: Arc::new(RwLock::new(stream)),
        }
    }
}

impl<R> Clone for SharedStream<R> {
    fn clone(&self) -> Self {
        SharedStream {
            stream: self.stream.clone(),
        }
    }
}

impl<R: Read> Read for SharedStream<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        match self.stream.write() {
            Ok(mut it) => Ok(it.read(buffer)?),
            Err(_) => Err(std::io::ErrorKind::Interrupted.into()),
        }
    }
}

impl<W: Write> Write for SharedStream<W> {
    fn write(&mut self, buffer: &[u8]) -> std::result::Result<usize, std::io::Error> {
        match self.stream.write() {
            Ok(mut it) => Ok(it.write(buffer)?),
            Err(_) => Err(std::io::ErrorKind::Interrupted.into()),
        }
    }

    fn flush(&mut self) -> std::result::Result<(), std::io::Error> {
        match self.stream.write() {
            Ok(mut it) => Ok(it.flush()?),
            Err(_) => Err(std::io::ErrorKind::Interrupted.into()),
        }
    }
}
