#![allow(dead_code)]

use std::str::{from_utf8, from_utf8_unchecked};
use std::net::{TcpStream};
use std::cell::{RefCell};

use crate::{Result};

pub fn from_utf8_forced(buffer: &[u8]) -> &str {
    match from_utf8(&buffer) {
        Ok(content) => content,
        Err(error) => unsafe {
            from_utf8_unchecked(&buffer[..error.valid_up_to()])
        }
    }
}

pub trait TcpSplit {
    fn split(self) -> Result<(TcpStream, TcpStream)>;
    fn split_to_refcells(self) -> Result<(RefCell<TcpStream>, RefCell<TcpStream>)>;
}

impl TcpSplit for TcpStream {
    fn split(self) -> Result<(TcpStream, TcpStream)> {
        Ok((self.try_clone()?, self))
    }

    fn split_to_refcells(self) -> Result<(RefCell<TcpStream>, RefCell<TcpStream>)> {
        let (writing, reading) = self.split()?;
        Ok((RefCell::new(writing), RefCell::new(reading)))
    }
}

pub fn with_refcell<F, V, T>(value: V, run: F) -> Result<(V, T)>
where
    F: FnOnce(&RefCell<V>) -> Result<T>
{
    let wrapped = RefCell::new(value);
    let result = run(&wrapped)?;
    Ok((wrapped.into_inner(), result))
}
