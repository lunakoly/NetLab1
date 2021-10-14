use std::fs::{File};

use crate::shared::map::{SharedMap};

pub struct FileSharer {
    pub name: String,
    pub path: String,
    pub file: File,
    pub size: usize,
    pub written: usize,
}

impl FileSharer {
    pub fn rest(&self) -> usize {
        self.size - self.written
    }
}

pub type FileSharers = SharedMap<String, FileSharer>;
