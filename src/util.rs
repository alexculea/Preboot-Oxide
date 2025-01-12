use crate::Result;
use std::collections::HashMap;

/// Thin wrapper around a HashMap that enforces a maximum number of elements.
pub struct QuotaMap<Left: std::cmp::Eq + std::hash::Hash, Right> {
    sessions: HashMap<Left, Right>,
    max_elements: u64,
}

impl<Left: std::cmp::Eq + std::hash::Hash, Right> QuotaMap<Left, Right> {
    pub fn new(max_elements: u64) -> Self {
        Self {
            sessions: Default::default(),
            max_elements,
        }
    }

    pub fn insert(&mut self, key: Left, value: Right) -> Result<()> {
        if u64::try_from(self.sessions.len())? > self.max_elements {
            bail!("Max quota of elements {} reached.", self.max_elements)
        }

        self.sessions.insert(key, value);
        Ok(())
    }

    pub fn remove(&mut self, key: &Left) -> Option<Right> {
        self.sessions.remove(key)
    }

    pub fn get(&self, key: &Left) -> Option<&Right> {
        self.sessions.get(key)
    }

    pub fn get_mut(&mut self, key: &Left) -> Option<&mut Right> {
        self.sessions.get_mut(key)
    }

    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&Left, &mut Right) -> bool,
    {
        self.sessions.retain(f);
    }

    pub fn iter(&self) -> std::collections::hash_map::Iter<Left, Right> {
        self.sessions.iter()
    }
}
