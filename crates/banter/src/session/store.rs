//! A typed per-session store.
//!
//! Lets independent pieces share data without knowing about each other: a gate can
//! record who authenticated, `init` can read it, a hook can keep a counter.

use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

#[derive(Default)]
pub struct Store {
    map: HashMap<TypeId, Box<dyn Any + Send>>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    /// Store a value, returning the one it replaced.
    pub fn insert<T: Send + 'static>(&mut self, value: T) -> Option<T> {
        self.map
            .insert(TypeId::of::<T>(), Box::new(value))
            .and_then(|old| old.downcast().ok().map(|b| *b))
    }

    pub fn get<T: Send + 'static>(&self) -> Option<&T> {
        self.map.get(&TypeId::of::<T>())?.downcast_ref()
    }

    pub fn get_mut<T: Send + 'static>(&mut self) -> Option<&mut T> {
        self.map.get_mut(&TypeId::of::<T>())?.downcast_mut()
    }

    /// The stored value, inserting `T::default()` first if there is none.
    pub fn get_or_default<T: Default + Send + 'static>(&mut self) -> &mut T {
        self.map
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(T::default()))
            .downcast_mut()
            .expect("keyed by its own type id")
    }

    pub fn remove<T: Send + 'static>(&mut self) -> Option<T> {
        self.map
            .remove(&TypeId::of::<T>())
            .and_then(|b| b.downcast().ok().map(|b| *b))
    }

    pub fn contains<T: Send + 'static>(&self) -> bool {
        self.map.contains_key(&TypeId::of::<T>())
    }
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store")
            .field("len", &self.map.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_keyed_by_type() {
        let mut ext = Store::new();
        assert_eq!(ext.insert(1u32), None);
        assert_eq!(ext.insert(2u32), Some(1));
        ext.insert("name");
        *ext.get_or_default::<u64>() += 5;
        assert_eq!(ext.get::<u32>(), Some(&2));
        assert_eq!(ext.get::<&str>(), Some(&"name"));
        assert_eq!(ext.remove::<u64>(), Some(5));
        assert!(!ext.contains::<u64>());
    }
}
