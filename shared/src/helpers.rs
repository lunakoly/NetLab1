use crate::{Result};

use std::str::{from_utf8, from_utf8_unchecked};

use std::sync::{Arc, RwLock};

use std::collections::HashMap;

pub fn from_utf8_forced(buffer: &[u8]) -> &str {
    match from_utf8(&buffer) {
        Ok(content) => content,
        Err(error) => unsafe {
            from_utf8_unchecked(&buffer[..error.valid_up_to()])
        }
    }
}

pub type NamesMap = Arc<RwLock<HashMap<String, String>>>;

pub fn get_name(names: NamesMap, address: String) -> Result<String> {
    let it = names.read()?;

    let proper = if it.contains_key(&address) {
        it[&address].clone()
    } else {
        address
    };

    Ok(proper)
}

pub type SafeVec<T> = Arc<RwLock<Vec<T>>>;
