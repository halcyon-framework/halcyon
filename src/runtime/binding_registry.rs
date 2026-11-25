// ============================================================================
// SPEC-HAL-020C: Binding Registry — Layered Binding Tables
// ============================================================================

use crate::core::event::{BindingKey, KeyCode, Modifiers, PhysicalKeyCode};
use crate::runtime::actions::{ActionId, Shortcut};
use std::collections::HashMap;

// ============================================================================
// InputPattern
// ============================================================================

/// Normalized input pattern for binding lookup.
/// Canonical form: KeyCode + modifier flags.
/// This is the lookup key — one input pattern maps to at most one ActionId.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputPattern {
    pub key: BindingKey,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

impl InputPattern {
    pub fn from_key_modifiers(key: KeyCode, modifiers: Modifiers) -> Self {
        Self {
            key: key.into(),
            ctrl: modifiers.ctrl,
            alt: modifiers.alt,
            shift: modifiers.shift,
            meta: modifiers.meta,
        }
    }

    pub fn key_only(key: KeyCode) -> Self {
        Self {
            key: key.into(),
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
        }
    }
}

impl From<&Shortcut> for InputPattern {
    fn from(shortcut: &Shortcut) -> Self {
        Self {
            key: shortcut.key,
            ctrl: shortcut.ctrl,
            alt: shortcut.alt,
            shift: shortcut.shift,
            meta: shortcut.meta,
        }
    }
}

impl InputPattern {
    pub fn from_physical_key_modifiers(
        key: PhysicalKeyCode,
        modifiers: Modifiers,
    ) -> Result<Self, crate::core::event::UnknownPhysicalKey> {
        Shortcut::from_physical_key_and_modifiers(key, modifiers)
            .map(|shortcut| Self::from(&shortcut))
    }
}

// ============================================================================
// BindingLayer
// ============================================================================

/// Named binding layer. Later layers override earlier layers (within same priority class).
/// INV-ACT-BIND-TBL-1: Later tables override earlier ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BindingLayer {
    /// Engine-level defaults (lowest precedence)
    System = 0,
    /// Application-level defaults
    App = 1,
    /// User configuration overrides
    User = 2,
    /// Temporary runtime overrides (mode switching, modal rebinding)
    Runtime = 3,
}

// ============================================================================
// BindingPriority
// ============================================================================

/// Priority class. Hard > Normal > Soft regardless of layer.
/// INV-ACT-BIND-PRI-1: Hard bindings override all others.
/// INV-ACT-BIND-PRI-2: Soft bindings cannot override Hard or Normal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BindingPriority {
    Soft = 0,
    Normal = 1,
    Hard = 2,
}

// ============================================================================
// Binding
// ============================================================================

/// A single binding entry.
#[derive(Debug, Clone, Copy)]
pub struct Binding {
    pub pattern: InputPattern,
    pub action_id: ActionId,
    pub layer: BindingLayer,
    pub priority: BindingPriority,
    /// Optional tag for batch removal (e.g., "navigation_mode", "capture_mode").
    /// Runtime layers use this to add/remove groups of bindings atomically.
    pub tag: Option<&'static str>,
}

// ============================================================================
// ResolvedBinding (internal)
// ============================================================================

#[derive(Debug, Clone, Copy)]
struct ResolvedBinding {
    action_id: ActionId,
    layer: BindingLayer,
    priority: BindingPriority,
    ordinal: usize,
}

// ============================================================================
// BindingRegistry
// ============================================================================

#[derive(Debug, Clone)]
pub struct BindingRegistry {
    /// All registered bindings, stored by layer.
    layers: [Vec<Binding>; 4], // indexed by BindingLayer as usize

    /// Flattened resolved lookup table. Rebuilt on mutation.
    /// INV-ACT-BIND-PERF-1: O(1) lookup.
    /// INV-ACT-BIND-PERF-2: No allocation per key event.
    resolved: HashMap<InputPattern, ResolvedBinding>,

    /// Dirty flag — true when layers changed but resolved table not yet rebuilt.
    dirty: bool,
}

impl BindingRegistry {
    pub fn new() -> Self {
        Self {
            layers: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            resolved: HashMap::new(),
            dirty: false,
        }
    }

    /// Add a binding to its layer, mark dirty.
    pub fn add(&mut self, binding: Binding) {
        let layer_idx = binding.layer as usize;
        self.layers[layer_idx].push(binding);
        self.dirty = true;
    }

    /// Remove all bindings with matching tag from all layers, mark dirty.
    /// This is the mechanism for runtime layer swapping.
    pub fn remove_by_tag(&mut self, tag: &str) {
        for layer in &mut self.layers {
            layer.retain(|b| b.tag.map_or(true, |t| t != tag));
        }
        self.dirty = true;
    }

    /// Clear entire Runtime layer, mark dirty.
    pub fn remove_all_runtime(&mut self) {
        self.layers[BindingLayer::Runtime as usize].clear();
        self.dirty = true;
    }

    /// Rebuild resolved table from layers. Conflict resolution:
    /// 1. Higher BindingPriority wins unconditionally
    /// 2. Within same priority, higher BindingLayer wins
    /// 3. Within same priority and layer, last-added wins (stable)
    /// Result: exactly one ActionId per InputPattern
    /// (INV-ACT-BIND-CONF-1, FORBID-ACT-BIND-MULTI)
    pub fn resolve(&mut self) {
        self.resolved.clear();

        // Process layers in order (System=0, App=1, User=2, Runtime=3).
        // Within each layer, process bindings in insertion order.
        for layer_idx in 0..4 {
            for (ordinal, binding) in self.layers[layer_idx].iter().enumerate() {
                let dominated = if let Some(existing) = self.resolved.get(&binding.pattern) {
                    // Higher priority wins unconditionally.
                    // Within same priority, higher layer wins.
                    // Within same priority and layer, last-added wins (replace).
                    binding.priority < existing.priority
                } else {
                    false
                };

                if !dominated {
                    self.resolved.insert(
                        binding.pattern,
                        ResolvedBinding {
                            action_id: binding.action_id,
                            layer: binding.layer,
                            priority: binding.priority,
                            ordinal,
                        },
                    );
                }
            }
        }

        self.dirty = false;
    }

    /// O(1) lookup. Calls resolve() if dirty.
    /// Returns Option<ActionId>.
    pub fn lookup(&mut self, pattern: &InputPattern) -> Option<ActionId> {
        if self.dirty {
            self.resolve();
        }
        self.resolved.get(pattern).map(|r| r.action_id)
    }

    /// Resolve logical and physical candidates once, with the ordinary precedence.
    pub fn lookup_key(
        &mut self,
        logical: KeyCode,
        physical: PhysicalKeyCode,
        modifiers: Modifiers,
    ) -> Option<ActionId> {
        if self.dirty {
            self.resolve();
        }
        let logical = (logical != KeyCode::Unknown)
            .then(|| {
                self.resolved
                    .get(&InputPattern::from_key_modifiers(logical, modifiers))
            })
            .flatten();
        let physical = InputPattern::from_physical_key_modifiers(physical, modifiers)
            .ok()
            .and_then(|pattern| self.resolved.get(&pattern));
        logical
            .into_iter()
            .chain(physical)
            .max_by_key(|binding| (binding.priority, binding.layer, binding.ordinal))
            .map(|binding| binding.action_id)
    }

    /// Convenience check.
    pub fn has_binding(&mut self, pattern: &InputPattern) -> bool {
        self.lookup(pattern).is_some()
    }
}

impl Default for BindingRegistry {
    fn default() -> Self {
        Self::new()
    }
}
