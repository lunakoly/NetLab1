use std::io::{Read};

pub trait CappedRead: Read {
    fn clear(&mut self);
}

pub struct CappedReader<R> {
    capacity: usize,
    stream: R,
    offset: usize,
}

pub trait IntoCappedReader<R: Read> {
    fn to_capped(self, capacity: usize) -> CappedReader<R>;
}

impl<R: Read> IntoCappedReader<R> for R {
    fn to_capped(self, capacity: usize) -> CappedReader<R> {
        CappedReader {
            capacity: capacity,
            stream: self,
            offset: 0,
        }
    }
}

impl<R: Read> Read for CappedReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, std::io::Error> {
        let max_allowed_count = std::cmp::min(
            self.capacity - self.offset,
            buffer.len()
        );

        let read_count = if max_allowed_count > 0 {
            self.stream.read(&mut buffer[..max_allowed_count])?
        } else {
            return Err(std::io::ErrorKind::InvalidData.into());
        };

        if read_count == 0 {
            return Ok(0);
        }

        self.offset += read_count;
        Ok(read_count)
    }
}

impl<R: Read> CappedRead for CappedReader<R> {
    fn clear(&mut self) {
        self.offset = 0;
    }
}
