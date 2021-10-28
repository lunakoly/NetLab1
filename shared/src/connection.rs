pub mod messages;
pub mod sharers;
pub mod helpers;

use std::net::{TcpStream, SocketAddr};
use std::io::{Write};
use std::fs::{File};
use std::cmp::{min};

use crate::{Result};
use crate::shared::{Shared};

use sharers::{FileSharer, FileSharers};

pub struct Context {
    stream: Shared<TcpStream>,
    reading_sharers: FileSharers,
    sending_sharers: Shared<Vec<FileSharer>>,
    nexd_id: usize,
}

impl Context {
    pub fn new(
        stream: Shared<TcpStream>,
        reading_sharers: FileSharers,
        sending_sharers: Shared<Vec<FileSharer>>,
    ) -> Context {
        Context {
            stream: stream,
            reading_sharers: reading_sharers,
            sending_sharers: sending_sharers,
            nexd_id: 0,
        }
    }
}

pub trait Connection {
    fn remote_address(&self) -> Result<SocketAddr>;

    fn free_id(&mut self) -> Result<usize>;

    fn prepare_sharer(
        &mut self,
        path: &str,
        file: File,
        name: &str,
    ) -> Result<()>;

    fn promote_sharer(
        &mut self,
        name: &str,
        size: usize,
        id: usize,
    ) -> Result<()>;

    fn accept_chunk(
        &mut self,
        data: &[u8],
        id: usize,
    ) -> Result<bool>;

    fn remove_unpromoted_sharer(&mut self, name: &str) -> Result<Option<FileSharer>>;

    fn remove_sharer(&mut self, id: usize) -> Result<Option<FileSharer>>;

    fn enqueu_sending_sharer(&mut self, sharer: FileSharer) -> Result<()>;

    fn sending_sharers_queue(&self) -> Result<Shared<Vec<FileSharer>>>;
}

impl Connection for Context {
    fn remote_address(&self) -> Result<SocketAddr> {
        Ok(self.stream.inner.read()?.peer_addr()?)
    }

    fn free_id(&mut self) -> Result<usize> {
        let id = self.nexd_id;
        self.nexd_id += 1;
        Ok(id)
    }

    fn prepare_sharer(
        &mut self,
        path: &str,
        file: File,
        name: &str,
    ) -> Result<()> {
        let sharer = FileSharer::new(name, path, file, 0, 0);
        self.reading_sharers.insert(name.to_owned(), sharer)?;
        Ok(())
    }

    fn promote_sharer(
        &mut self,
        name: &str,
        size: usize,
        id: usize,
    ) -> Result<()> {
        let mut sharer = match self.reading_sharers.remove(name)? {
            Some(it) => it,
            None => return Ok(())
        };

        sharer.size = size;
        sharer.id = id;

        let key = format!("{}", id);
        self.reading_sharers.insert(key, sharer)?;

        Ok(())
    }

    fn accept_chunk(
        &mut self,
        data: &[u8],
        id: usize,
    ) -> Result<bool> {
        let mut sharers = self.reading_sharers.write()?;
        let key = format!("{}", id);

        let sharer = if let Some(it) = sharers.get_mut(&key) {
            it
        } else {
            return Ok(false)
        };

        let writing_count = min(data.len(), sharer.rest());

        sharer.file.write(&data[..writing_count])?;
        sharer.written += writing_count;

        Ok(sharer.written >= sharer.size)
    }

    fn remove_unpromoted_sharer(&mut self, name: &str) -> Result<Option<FileSharer>> {
        self.reading_sharers.remove(name)
    }

    fn remove_sharer(&mut self, id: usize) -> Result<Option<FileSharer>> {
        let key = format!("{}", id);
        self.reading_sharers.remove(&key)
    }

    fn enqueu_sending_sharer(&mut self, sharer: FileSharer) -> Result<()> {
        self.sending_sharers.write()?.push(sharer);
        Ok(())
    }

    fn sending_sharers_queue(&self) -> Result<Shared<Vec<FileSharer>>> {
        Ok(self.sending_sharers.clone())
    }
}

pub trait WithConnection {
    fn connection(&self) -> &dyn Connection;
    fn connection_mut(&mut self) -> &mut dyn Connection;
}

impl<W: WithConnection> Connection for W {
    fn remote_address(&self) -> Result<SocketAddr> {
        self.connection().remote_address()
    }

    fn free_id(&mut self) -> Result<usize> {
        self.connection_mut().free_id()
    }

    fn prepare_sharer(
        &mut self,
        path: &str,
        file: File,
        name: &str,
    ) -> Result<()> {
        self.connection_mut().prepare_sharer(path, file, name)
    }

    fn promote_sharer(
        &mut self,
        name: &str,
        size: usize,
        id: usize,
    ) -> Result<()> {
        self.connection_mut().promote_sharer(name, size, id)
    }

    fn accept_chunk(
        &mut self,
        data: &[u8],
        id: usize,
    ) -> Result<bool> {
        self.connection_mut().accept_chunk(data, id)
    }

    fn remove_unpromoted_sharer(&mut self, name: &str) -> Result<Option<FileSharer>> {
        self.connection_mut().remove_unpromoted_sharer(name)
    }

    fn remove_sharer(&mut self, id: usize) -> Result<Option<FileSharer>> {
        self.connection_mut().remove_sharer(id)
    }

    fn enqueu_sending_sharer(&mut self, sharer: FileSharer) -> Result<()> {
        self.connection_mut().enqueu_sending_sharer(sharer)
    }

    fn sending_sharers_queue(&self) -> Result<Shared<Vec<FileSharer>>> {
        self.connection().sending_sharers_queue()
    }
}

impl<T: Connection> Connection for Shared<T> {
    fn remote_address(&self) -> Result<SocketAddr> {
        self.inner.read()?.remote_address()
    }

    fn free_id(&mut self) -> Result<usize> {
        self.inner.write()?.free_id()
    }

    fn prepare_sharer(
        &mut self,
        path: &str,
        file: File,
        name: &str,
    ) -> Result<()> {
        self.inner.write()?.prepare_sharer(path, file, name)
    }

    fn promote_sharer(
        &mut self,
        name: &str,
        size: usize,
        id: usize,
    ) -> Result<()> {
        self.inner.write()?.promote_sharer(name, size, id)
    }

    fn accept_chunk(
        &mut self,
        data: &[u8],
        id: usize,
    ) -> Result<bool> {
        self.inner.write()?.accept_chunk(data, id)
    }

    fn remove_unpromoted_sharer(&mut self, name: &str) -> Result<Option<FileSharer>> {
        self.inner.write()?.remove_unpromoted_sharer(name)
    }

    fn remove_sharer(&mut self, id: usize) -> Result<Option<FileSharer>> {
        self.inner.write()?.remove_sharer(id)
    }

    fn enqueu_sending_sharer(&mut self, sharer: FileSharer) -> Result<()> {
        self.inner.write()?.enqueu_sending_sharer(sharer)
    }

    fn sending_sharers_queue(&self) -> Result<Shared<Vec<FileSharer>>> {
        self.inner.write()?.sending_sharers_queue()
    }
}
