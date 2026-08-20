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

    // Take ownership of a registered adapter by id, consuming the registry.
    //
    // Frontends that share one adapter across threads (for example the
    // desktop bridge) need owned access rather than a borrow.
    pub fn into_adapter(mut self, id: &str) -> Option<Box<dyn HarnessAdapter>> {
        let index = self
            .adapters
            .iter()
            .position(|adapter| adapter.metadata().id == id)?;
        Some(self.adapters.remove(index))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::FakeAdapter;

    #[test]
    fn into_adapter_transfers_ownership() {
        let mut registry = AdapterRegistry::new();
        registry.register(Box::new(FakeAdapter::healthy()));
        let adapter = registry.into_adapter("test").expect("adapter registered");
        assert_eq!(adapter.metadata().id, "test");
    }

    #[test]
    fn into_adapter_returns_none_for_unknown_id() {
        let registry = AdapterRegistry::new();
        assert!(registry.into_adapter("missing").is_none());
    }
}
