//! Hit Region Utilities for IR Elements
//!
//! This module provides utilities for mapping between ViewNodeId and hit region IDs.
//! Hit regions are used by the engine's hit testing system to detect clicks on elements.
//!
//! # Approach
//!
//! We use a simple hash-based mapping:
//! - ViewNodeId (String) → u32 region ID via hashing
//! - Region ID → ViewNodeId via a lookup table
//!
//! This allows the hit testing system to return region IDs, which we can then
//! map back to the original ViewNodeId to dispatch events.

use rune_ir::view::ViewNodeId;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Registry for mapping between ViewNodeId and hit region IDs
///
/// This structure maintains bidirectional mapping between IR view nodes
/// and hit region IDs used by the engine's hit testing system.
pub struct HitRegionRegistry {
    /// Map from ViewNodeId to hit region ID
    node_to_region: HashMap<ViewNodeId, u32>,

    /// Map from hit region ID back to ViewNodeId
    region_to_node: HashMap<u32, ViewNodeId>,

    /// Counter for generating unique region IDs (fallback if hash collision)
    next_region_id: u32,
}

impl HitRegionRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            node_to_region: HashMap::new(),
            region_to_node: HashMap::new(),
            next_region_id: 1000, // Start at 1000 to avoid conflicts with system regions
        }
    }

    /// Clear all mappings
    pub fn clear(&mut self) {
        self.node_to_region.clear();
        self.region_to_node.clear();
        self.next_region_id = 1000;
    }

    /// Register a ViewNodeId and get its hit region ID
    ///
    /// If the node was already registered, returns the existing region ID.
    /// Otherwise, generates a new region ID (via hash or unique counter).
    pub fn register(&mut self, view_node_id: &ViewNodeId) -> u32 {
        // Check if already registered
        if let Some(&region_id) = self.node_to_region.get(view_node_id) {
            return region_id;
        }

        // Generate region ID from hash
        let region_id = self.hash_to_region_id(view_node_id);

        // Check for collision
        let region_id = if self.region_to_node.contains_key(&region_id) {
            // Collision detected, use sequential ID
            let id = self.next_region_id;
            self.next_region_id += 1;
            id
        } else {
            region_id
        };

        // Store bidirectional mapping
        self.node_to_region.insert(view_node_id.clone(), region_id);
        self.region_to_node.insert(region_id, view_node_id.clone());

        region_id
    }

    /// Look up the ViewNodeId for a given hit region ID
    pub fn lookup(&self, region_id: u32) -> Option<&ViewNodeId> {
        self.region_to_node.get(&region_id)
    }

    /// Check if a region ID is registered
    pub fn contains_region(&self, region_id: u32) -> bool {
        self.region_to_node.contains_key(&region_id)
    }

    /// Hash a ViewNodeId to a u32 region ID
    fn hash_to_region_id(&self, view_node_id: &ViewNodeId) -> u32 {
        let mut hasher = DefaultHasher::new();
        view_node_id.hash(&mut hasher);
        let hash = hasher.finish();

        // Use upper 32 bits to avoid collision with system region IDs (0-999)
        ((hash >> 32) as u32).max(1000)
    }
}

impl Default for HitRegionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_lookup() {
        let mut registry = HitRegionRegistry::new();

        let node_id = "test-node-1".to_string();
        let region_id = registry.register(&node_id);

        assert!(region_id >= 1000, "Region ID should be >= 1000");
        assert_eq!(registry.lookup(region_id), Some(&node_id));
    }

    #[test]
    fn test_duplicate_registration() {
        let mut registry = HitRegionRegistry::new();

        let node_id = "test-node-1".to_string();
        let region_id_1 = registry.register(&node_id);
        let region_id_2 = registry.register(&node_id);

        assert_eq!(
            region_id_1, region_id_2,
            "Should return same region ID for duplicate registration"
        );
    }

    #[test]
    fn test_clear() {
        let mut registry = HitRegionRegistry::new();

        let node_id = "test-node-1".to_string();
        let region_id = registry.register(&node_id);

        assert!(registry.contains_region(region_id));

        registry.clear();

        assert!(!registry.contains_region(region_id));
    }
}
