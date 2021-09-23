use shared::helpers;

use std::option::Option;

use std::str::Chars;

use std::io::BufRead;

pub struct CharsReader<'a> {
    stream: &'a mut dyn BufRead,
    cache: Chars<'a>,
}

pub trait IntoCharsReader<'a> {
    fn chars(self) -> CharsReader<'a>;
}

impl<'a> IntoCharsReader<'a> for &'a mut dyn BufRead {
    fn chars(self) -> CharsReader<'a> {
        CharsReader {
            stream: self,
            cache: "".chars(),
        }
    }
}

impl<'a> Iterator for CharsReader<'a>  {
    type Item = char;

    fn next<'d>(&'d mut self) -> Option<Self::Item> {
        if let Some(it) = self.cache.next() {
            self.stream.consume(it.len_utf8());
            return Some(it);
        }

        let buffer = match self.stream.fill_buf() {
            Ok(it) => unsafe {
                // TODO: review
                std::mem::transmute::<&[u8], &'a [u8]>(it)
            }
            Err(_) => return None
        };

        self.cache = helpers::from_utf8_forced(&buffer).chars();

        return match self.cache.next() {
            Some(it) => {
                self.stream.consume(it.len_utf8());
                Some(it)
            }
            None => None
        }
    }
}
