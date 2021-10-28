use std::fs::{File};

use crate::shared::map::{SharedMap};

use chrono::{Local, DateTime};

pub struct FileSharer {
    pub name: String,
    pub path: String,
    pub file: File,
    pub size: usize,
    pub id: usize,
    pub written: usize,
    pub old_time_point: DateTime<Local>,
}

impl FileSharer {
    pub fn new(
        name: &str,
        path: &str,
        file: File,
        size: usize,
        id: usize,
    ) -> FileSharer {
        FileSharer {
            name: name.to_owned(),
            path: path.to_owned(),
            file: file,
            size: size,
            id: id,
            written: 0,
            old_time_point: Local::now()
        }
    }

    pub fn rest(&self) -> usize {
        self.size - self.written
    }

    pub fn percentage(&self) -> u8 {
        (self.written * 100 / self.size) as u8
    }
}

pub type FileSharers = SharedMap<String, FileSharer>;
