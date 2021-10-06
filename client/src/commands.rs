use std::iter::{Peekable};
use std::net::{TcpStream};
use std::cell::{RefCell};

use crate::chars_reader::{CharsReader};

use shared::communication::{DEFAULT_PORT};

pub enum Command {
    Nothing,
    End,
    Text { text: String },
    Rename { new_name: String },
    Connect { address: String },
}

pub enum CommandProcessing {
    Proceed,
    Stop,
    Connect(RefCell<TcpStream>)
}

fn is_blank(symbol: char) -> bool {
    symbol == '\r' ||
    symbol == '\n' ||
    symbol == '\t' ||
    symbol == ' '
}

fn parse_rename(words: &[String]) -> Command {
    if words.len() >= 2 {
        Command::Rename {
            new_name: words[1].clone(),
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
    } else if words[0] == "/rename" {
        parse_rename(&words)
    } else if words[0] == "/connect" || words[0] == "/c" {
        parse_connect(&words)
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
