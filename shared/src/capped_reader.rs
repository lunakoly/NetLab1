use std::io::Read;

pub trait CappedRead: Read {
    fn clear(&mut self);
}

const CAPPED_READER_CAPACITY: usize = 100;

pub struct CappedReader<R> {
    stream: R,
    buffer: [u8; CAPPED_READER_CAPACITY],
    length: usize,
    offset: usize,
}

pub trait IntoCappedReader<R: Read> {
    fn capped(self) -> CappedReader<R>;
}

impl<R: Read> IntoCappedReader<R> for R {
    fn capped(self) -> CappedReader<R> {
        CappedReader {
            stream: self,
            buffer: [0; CAPPED_READER_CAPACITY],
            length: 0,
            offset: 0,
        }
    }
}

impl<R: Read> Read for CappedReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, std::io::Error> {
        if self.offset == self.length {
            if self.length < CAPPED_READER_CAPACITY {
                self.length += self.stream.read(&mut self.buffer[self.length..])?;
            } else {
                return Err(std::io::ErrorKind::InvalidData.into());
            }

            // Only possible if we've read 0
            // bytes from the underlying stream
            if self.offset == self.length {
                return Ok(0);
            }
        }

        let copy_count = std::cmp::min(self.length - self.offset, buffer.len());
        let tail = self.offset + copy_count;

        let copy_result = std::io::copy(
            &mut &self.buffer[self.offset..tail],
            &mut &mut buffer[..copy_count]
        );

        match copy_result {
            Ok(actually_copied) => {
                self.offset += actually_copied as usize;
                Ok(actually_copied as usize)
            }
            Err(error) => {
                Err(error)
            }
        }
    }
}

impl<R: Read> CappedRead for CappedReader<R> {
    fn clear(&mut self) {
        self.length -= self.offset;
        self.buffer.rotate_left(self.offset);
        self.offset = 0;
    }
}
