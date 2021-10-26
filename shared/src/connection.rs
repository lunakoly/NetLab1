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
    sharers: FileSharers,
    nexd_id: usize,
}

impl Context {
    pub fn new(stream: Shared<TcpStream>, sharers: FileSharers) -> Context {
        Context {
            stream: stream,
            sharers: sharers,
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

    fn sharers_map(&mut self) -> Result<FileSharers>;
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
        let sharer = FileSharer {
            name: name.to_owned(),
            path: path.to_owned(),
            file: file,
            size: 0,
            written: 0,
        };

        self.sharers.insert(name.to_owned(), sharer)?;
        Ok(())
    }

    fn promote_sharer(
        &mut self,
        name: &str,
        size: usize,
        id: usize,
    ) -> Result<()> {
        let mut sharer = match self.sharers.remove(name)? {
            Some(it) => it,
            None => return Ok(())
        };

        sharer.size = size;

        let key = format!("{}", id);
        self.sharers.insert(key, sharer)?;

        Ok(())
    }

    fn accept_chunk(
        &mut self,
        data: &[u8],
        id: usize,
    ) -> Result<bool> {
        let mut sharers = self.sharers.write()?;
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
        self.sharers.remove(name)
    }

    fn remove_sharer(&mut self, id: usize) -> Result<Option<FileSharer>> {
        let key = format!("{}", id);
        self.sharers.remove(&key)
    }

    fn sharers_map(&mut self) -> Result<FileSharers> {
        Ok(self.sharers.clone())
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

    fn sharers_map(&mut self) -> Result<FileSharers> {
        self.connection_mut().sharers_map()
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

    fn sharers_map(&mut self) -> Result<FileSharers> {
        self.inner.write()?.sharers_map()
    }
}
