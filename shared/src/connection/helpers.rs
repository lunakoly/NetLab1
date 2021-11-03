use std::io::{Read, Seek, SeekFrom};

use crate::{Result, is_would_block_error};
use crate::errors::{with_error_report};

use crate::communication::{
    WriteMessage,
};

use super::messages::{CommonMessage, CHUNK_SIZE};
use super::sharers::{FileSharer};
use super::{Connection};

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

    let chunk = CommonMessage::Chunk {
        data: buffer[..read].to_vec(),
        id: sharer.id,
    };

    let result = writer.write_message(&chunk);

    if let Err(error) = result {
        sharer.file.seek(SeekFrom::Start(sharer.written as u64))?;
        return Err(error);
    }

    sharer.written += read;

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

pub fn process_sending_sharers<C>(
    connection: &mut C,
) -> Result<bool>
where
    C: Connection + WriteMessage<CommonMessage>,
{
    let sending_sharers = connection.sending_sharers_queue()?;

    if sending_sharers.read()?.len() == 0 {
        return Ok(false)
    }

    let mut to_be_removed = vec![];

    for (index, it) in sending_sharers.write()?.iter_mut().enumerate() {
        let result = send_chunk(connection, it);

        if let Err(error) = result {
            if !is_would_block_error(&error) {
                return Err(error)
            }
            // Chill
        } else if it.rest() == 0 {
            to_be_removed.push(index);
        }
    }

    let mut removed_count = 0usize;

    for it in to_be_removed {
        sending_sharers.write()?.remove(it - removed_count);
        removed_count += 1;
    }

    Ok(true)
}
