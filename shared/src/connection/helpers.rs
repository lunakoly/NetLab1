use std::io::{Read};

use crate::{Result};
use crate::errors::{with_error_report};

use crate::communication::{
    WriteMessage,
};

use super::messages::{CommonMessage, CHUNK_SIZE};
use super::sharers::{FileSharer};

use chrono::{Local};

pub fn send_chunk<W>(
    writer: &mut W,
    sharer: &mut FileSharer,
) -> Result<()>
where
    W: WriteMessage<CommonMessage>
{
    let mut buffer = [0u8; CHUNK_SIZE];
    let read = sharer.file.read(&mut buffer)?;
    sharer.written += read;

    let chunk = CommonMessage::Chunk {
        data: buffer.to_vec(),
        id: sharer.id,
    };

    writer.write_message(&chunk)?;

    let time_point = Local::now();

    if (time_point - sharer.old_time_point).num_seconds() >= 1 {
        sharer.old_time_point = time_point;
        println!("(Console) File {} > {}%", sharer.name, sharer.percentage());
    }

    Ok(())
}

pub fn send_file<W>(
    writer: &mut W,
    sharer: &mut FileSharer,
) -> Result<()>
where
    W: WriteMessage<CommonMessage>
{
    while sharer.rest() > 0 {
        send_chunk(writer, sharer)?;
    }

    Ok(())
}

pub fn send_file_non_blocking<W>(
    writer: &mut W,
    mut sharer: FileSharer,
) -> Result<()>
where
    W: WriteMessage<CommonMessage>
     + Clone
     + Send + Sync + 'static,
{
    let mut the_writer = writer.clone();

    std::thread::spawn(move || {
        with_error_report(|| send_file(&mut the_writer, &mut sharer));
    });

    Ok(())
}
