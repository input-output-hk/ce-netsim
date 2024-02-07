use crate::SimId;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

/// Name Service
///
/// Simple registry for services to register a name to be associated
/// to their ID for easy retrieval and better human configuration.
///
#[derive(Debug, Clone)]
pub struct NameService {
    ns: Arc<RwLock<HashMap<String, SimId>>>,
}

impl NameService {
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            ns: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// lookup an entry in the name service registry
    ///
    /// returns `None` if there are no Id associated to that name
    pub fn lookup(&self, name: &str) -> Option<SimId> {
        self.ns
            .read()
            .expect("We shouldn't have poisoning")
            .get(name)
            .copied()
    }

    /// register a new Name to Id.
    ///
    /// If the name was already registered then the new id replaces
    /// the existing value, returning the old one.
    pub fn register(&self, name: impl Into<String>, id: SimId) -> Option<SimId> {
        self.ns
            .write()
            .expect("We shouldn't have poisoning")
            .insert(name.into(), id)
    }
}
