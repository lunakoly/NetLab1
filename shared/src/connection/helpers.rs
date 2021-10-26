use std::io::{Read};
use std::fs::{File};

use crate::{Result};
use crate::errors::{with_error_report};

use crate::communication::{
    WriteMessage,
    SendFile,
};

use super::messages::{CommonMessage, CHUNK_SIZE};

use chrono::{Local};

impl<W> SendFile for W
where
    W: WriteMessage<CommonMessage>
     + Clone
     + Send + Sync + 'static,
{
    fn send_file(
        &mut self,
        file: &mut File,
        size: usize,
        id: usize
    ) -> Result<()> {
        let mut written = 0usize;
        let mut old_time_point = Local::now();

        while written < size {
            let mut buffer = [0u8; CHUNK_SIZE];
            let read = file.read(&mut buffer)?;
            written += read;

            let chunk = CommonMessage::Chunk {
                data: buffer.to_vec(),
                id: id.clone(),
            };

            self.write_message(&chunk)?;

            let time_point = Local::now();

            if (time_point - old_time_point).num_seconds() >= 1 {
                old_time_point = time_point;
                println!("(Console) File #{} > {}%", id.clone(), written * 100 / size);
            }
        }

        Ok(())
    }

    fn send_file_non_blocking(
        &mut self,
        file: File,
        size: usize,
        id: usize
    ) -> Result<()> {
        let the_writer = self.clone();

        std::thread::spawn(move || {
            let mut owned_file = file;
            let mut owned_writer = the_writer;

            with_error_report(|| -> Result<()> {
                owned_writer.send_file(&mut owned_file, size, id)
            });
        });

        Ok(())
    }
}
