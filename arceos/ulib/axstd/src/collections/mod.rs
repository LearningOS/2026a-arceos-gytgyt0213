//! Heap-allocated collections, including a hash table for kernel applications.

pub use alloc::collections::*;

pub mod hash_map;
pub use hash_map::HashMap;
