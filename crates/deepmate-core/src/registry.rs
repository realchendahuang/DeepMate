use crate::adapter::{AdapterMetadata, HarnessAdapter};

// A simple registry of available harness adapters.
#[derive(Default)]
pub struct AdapterRegistry {
    adapters: Vec<Box<dyn HarnessAdapter>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, adapter: Box<dyn HarnessAdapter>) {
        self.adapters.push(adapter);
    }

    pub fn get(&self, id: &str) -> Option<&dyn HarnessAdapter> {
        self.adapters
            .iter()
            .find(|adapter| adapter.metadata().id == id)
            .map(|adapter| adapter.as_ref())
    }

    pub fn list(&self) -> Vec<AdapterMetadata> {
        self.adapters
            .iter()
            .map(|adapter| adapter.metadata())
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }

    pub fn len(&self) -> usize {
        self.adapters.len()
    }
}
