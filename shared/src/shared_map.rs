use std::collections::{HashMap};
use std::sync::{Arc, RwLock};
use std::hash::{Hash};
use std::borrow::{Borrow};

use crate::{Result};

pub type SharedMap<K, V> = Arc<RwLock<HashMap<K, V>>>;

pub trait SimpleMap<K, V> {
    fn contains_key<Q>(&self, key: &Q) -> Result<bool>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized;

    fn get_clone<Q>(&self, key: &Q) -> Result<Option<V>>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
        V: Clone;

    fn remove<Q: ?Sized>(&self, key: &Q) -> Result<Option<V>>
    where
        K: Borrow<Q>,
        Q: Hash + Eq;

    fn insert(&self, key: K, value: V) -> Result<Option<V>>;
}

impl<K, V> SimpleMap<K, V> for SharedMap<K, V>
where
    K: Eq + Hash,
{
    fn contains_key<Q>(&self, key: &Q) -> Result<bool>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        Ok(self.read()?.contains_key(key))
    }

    fn get_clone<Q>(&self, key: &Q) -> Result<Option<V>>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
        V: Clone
    {
        match self.write()?.get(key) {
            Some(it) => Ok(Some(it.clone())),
            None => Ok(None)
        }
    }

    fn remove<Q: ?Sized>(&self, key: &Q) -> Result<Option<V>>
    where
        K: Borrow<Q>,
        Q: Hash + Eq
    {
        Ok(self.write()?.remove(key))
    }

    fn insert(&self, key: K, value: V) -> Result<Option<V>> {
        Ok(self.write()?.insert(key, value))
    }
}
