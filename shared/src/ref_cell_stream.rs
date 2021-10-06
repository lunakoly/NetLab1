use std::io::{Read, Write};
use std::cell::{RefCell};

pub struct RefCellStream<'a, R> {
    pub backend: &'a RefCell<R>,
}

impl<'a, R> RefCellStream<'a, R> {
    pub fn new(backend: &'a RefCell<R>) -> RefCellStream<'a, R> {
        RefCellStream {
            backend: backend,
        }
    }
}

impl<'a, R: Read> Read for RefCellStream<'a, R> {
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, std::io::Error> {
        let mut lock = self.backend.borrow_mut();
        lock.read(buffer)
    }
}

impl<'a, W: Write> Write for RefCellStream<'a, W> {
    fn write(&mut self, buffer: &[u8]) -> Result<usize, std::io::Error> {
        let mut lock = self.backend.borrow_mut();
        lock.write(buffer)
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        let mut lock = self.backend.borrow_mut();
        lock.flush()
    }
}
