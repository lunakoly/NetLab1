use std::iter::Peekable;

use crate::chars_reader::CharsReader;

pub enum Command {
    Nothing,
    Message { text: String },
}

fn parse_command<'a>(input: &mut Peekable<CharsReader<'a>>) -> Command {
    while let Some(it) = input.next() {
        if it == '\n' {
            break
        }
    }

    println!("Well, yea, you issued a command, but I missed it, sorry...");
    Command::Nothing
}

fn parse_message<'a>(input: &mut Peekable<CharsReader<'a>>) -> Command {
    let mut line = String::new();

    while let Some(it) = input.next() {
        if it != '\n' {
            line.push(it);
        } else {
            break
        }
    }

    Command::Message {
        text: line,
    }
}

pub fn parse<'a>(input: &mut Peekable<CharsReader<'a>>) -> Command {
    if input.peek() == Some(&'/') {
        parse_command(input)
    } else {
        parse_message(input)
    }
}
