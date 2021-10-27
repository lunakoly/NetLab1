pub mod map;

use std::io::{Read, Write};
use std::sync::{Arc, RwLock};

use crate::{Result};

use crate::communication::{
    ReadMessage,
    WriteMessage,
};

pub struct Shared<R> {
   pub inner: Arc<RwLock<R>>,
}

impl<R> Shared<R> {
    pub fn new(stream: R) -> Shared<R> {
        Shared {
            inner: Arc::new(RwLock::new(stream)),
        }
    }

    pub fn read(&self) -> std::result::Result<
        std::sync::RwLockReadGuard<'_, R>,
        std::sync::PoisonError<std::sync::RwLockReadGuard<'_, R>>
    > {
        self.inner.read()
    }

    pub fn write(&self) -> std::result::Result<
        std::sync::RwLockWriteGuard<'_, R>,
        std::sync::PoisonError<std::sync::RwLockWriteGuard<'_, R>>
    > {
        self.inner.write()
    }
}

pub trait IntoShared<T> {
    fn shared(self) -> Shared<T>;
}

impl<T> IntoShared<T> for T {
    fn shared(self) -> Shared<T> {
        Shared::new(self)
    }
}

impl<R> Clone for Shared<R> {
    fn clone(&self) -> Self {
        Shared {
            inner: self.inner.clone(),
        }
    }
}

impl<R: Read> Read for Shared<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        match self.inner.write() {
            Ok(mut it) => Ok(it.read(buffer)?),
            Err(_) => Err(std::io::ErrorKind::Interrupted.into()),
        }
    }
}

impl<W: Write> Write for Shared<W> {
    fn write(&mut self, buffer: &[u8]) -> std::result::Result<usize, std::io::Error> {
        match self.inner.write() {
            Ok(mut it) => Ok(it.write(buffer)?),
            Err(_) => Err(std::io::ErrorKind::Interrupted.into()),
        }
    }

    fn flush(&mut self) -> std::result::Result<(), std::io::Error> {
        match self.inner.write() {
            Ok(mut it) => Ok(it.flush()?),
            Err(_) => Err(std::io::ErrorKind::Interrupted.into()),
        }
    }
}

impl<M, R: ReadMessage<M>> ReadMessage<M> for Shared<R> {
    fn read_message(&mut self) -> Result<M> {
        self.inner.write()?.read_message()
    }
}

impl<M, W: WriteMessage<M>> WriteMessage<M> for Shared<W> {
    fn write_message(&mut self, message: &M) -> Result<()> {
        self.inner.write()?.write_message(message)
    }
}
