use std::iter::{Peekable};
use std::path::{Path};

use crate::chars_reader::{CharsReader};

use super::{ArsonClientSession};

use shared::communication::{DEFAULT_PORT};
use shared::connection::messages::{MAXIMUM_TEXT_SIZE, MAXIMUM_NAME_SIZE};

pub enum Command {
    Nothing,
    End,
    Text { text: String },
    Rename { new_name: String },
    Connect { address: String },
    UploadFile { name: String, path: String },
    DownloadFile { name: String, path: String },
}

pub enum CommandProcessing {
    Proceed,
    Stop,
    Connect(ArsonClientSession)
}

fn is_blank(symbol: char) -> bool {
    symbol == '\r' ||
    symbol == '\n' ||
    symbol == '\t' ||
    symbol == ' '
}

fn parse_rename(words: &[String]) -> Command {
    if words.len() >= 2 {
        if words[1].len() > MAXIMUM_NAME_SIZE {
            println!("(Console) No way, sorry, this is way too long");
            Command::Nothing
        } else {
            Command::Rename {
                new_name: words[1].clone(),
            }
        }
    } else {
        println!("(Console) Rename to who? Vasya, Petia - who exactly?");
        Command::Nothing
    }
}

fn parse_connect(words: &[String]) -> Command {
    if words.len() >= 3 {
        Command::Connect {
            address: format!("{}:{}", words[1], words[2])
        }
    } else if words.len() >= 2 {
        Command::Connect {
            address: format!("{}:{}", words[1], DEFAULT_PORT),
        }
    } else {
        Command::Connect {
            address: format!("localhost:{}", DEFAULT_PORT),
        }
    }
}

fn check_upload(path: String, name: String) -> Command {
    if !Path::new(&path).exists() {
        println!("(Console) No, I can't find such a file");
        return Command::Nothing;
    }

    Command::UploadFile { path, name }
}

fn parse_upload(words: &[String]) -> Command {
    if words.len() >= 3 {
        check_upload(
            words[1].clone(),
            words[2].clone()
        )
    } else if words.len() >= 2 {
        check_upload(
            words[1].clone(),
            words[1].rsplit('/').next().unwrap_or("unnamed.txt").to_owned()
        )
    } else {
        println!("(Console) Everyone keeps telling 'send a file', 'get a file', but only the few of them actually know the right path");
        Command::Nothing
    }
}

fn check_download(path: String, name: String) -> Command {
    if Path::new(&path).exists() {
        println!("(Console) No, wait, the file already exists!");
        return Command::Nothing;
    }

    Command::DownloadFile { path, name }
}

fn parse_download(words: &[String]) -> Command {
    if words.len() >= 3 {
        check_download(
            words[2].clone(),
            words[1].clone(),
        )
    } else if words.len() >= 2 {
        check_download(
            words[1].clone(),
            words[1].clone(),
        )
    } else {
        println!("(Console) There's one crucial ingredient missing. Go on, try to find it out");
        Command::Nothing
    }
}

fn parse_words<'a>(input: &mut Peekable<CharsReader<'a>>) -> Vec<String> {
    let mut words = vec!["".to_owned()];

    while let Some(it) = input.next() {
        if it == '\r' {
            // ignore
        } else if it == '\n' {
            break
        } else if is_blank(it) {
            if words[words.len() - 1].len() != 0 {
                words.push("".to_owned());
            }
        } else {
            let last_index = words.len() - 1;
            words[last_index].push(it);
        }
    }

    words
}

fn parse_command<'a>(input: &mut Peekable<CharsReader<'a>>) -> Command {
    let words = parse_words(input);

    if words[0] == "/" {
        println!("(Console) Nooooooo, you can't just put a blank symbol after the '/'!!!!!!");
        Command::Nothing
    } else if words[0] == "/q" || words[0] == "/quit" || words[0] == "/exit" {
        Command::End
    } else if words[0] == "/rename" || words[0] == "/r" {
        parse_rename(&words)
    } else if words[0] == "/connect" || words[0] == "/c" {
        parse_connect(&words)
    } else if words[0] == "/upload" || words[0] == "/u" {
        parse_upload(&words)
    } else if words[0] == "/download" || words[0] == "/d" {
        parse_download(&words)
    } else {
        println!("(Console) Well, yea, you issued a command, but I missed it, sorry...");
        Command::Nothing
    }
}

fn parse_text<'a>(input: &mut Peekable<CharsReader<'a>>) -> Command {
    let mut line = String::new();

    while let Some(it) = input.next() {
        if it == '\r' {
            // ignore
        } else if it != '\n' {
            line.push(it);
        } else {
            break
        }
    }

    if line.is_empty() {
        Command::Nothing
    } else if line.len() > MAXIMUM_TEXT_SIZE {
        println!("(Console) No way, sorry, this is way too long");
        Command::Nothing
    } else {
        Command::Text {
            text: line,
        }
    }
}

pub fn parse<'a>(input: &mut Peekable<CharsReader<'a>>) -> Command {
    if input.peek() == Some(&'/') {
        parse_command(input)
    } else if let Some(_) = input.peek() {
        parse_text(input)
    } else {
        Command::End
    }
}
