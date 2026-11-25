// =============================================================================
// View Configuration Persistence Infrastructure
// =============================================================================
//
// Instance-scoped, topology-aware, migration-tolerant persistence for column
// configuration. Widget-agnostic: ListView, TreeView, Grid all use the same
// free functions and store trait.
//
// No version structs (no V1/V2/schema_version). Evolution via normalization
// + reconciliation + tolerant deserialization.
// =============================================================================

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::column::ColumnModel;
use super::column_config::ColumnConfigurationState;
use super::data_adapter::SortSpec;

// =============================================================================
// ViewConfigKey — Instance-scoped identity
// =============================================================================

/// Instance-scoped configuration identity.
///
/// Storage key format: `"{app_namespace}:{widget_type}:{view_id}"`.
/// If `view_id` is empty or not provided, persistence is disabled.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ViewConfigKey {
    pub app_namespace: String,
    pub widget_type: String,
    pub view_id: String,
}

impl ViewConfigKey {
    /// Build the storage key string.
    pub fn storage_key(&self) -> String {
        format!(
            "{}:{}:{}",
            self.app_namespace, self.widget_type, self.view_id
        )
    }

    /// Check if this key enables persistence (view_id must be non-empty).
    pub fn is_persistence_enabled(&self) -> bool {
        !self.view_id.is_empty()
    }
}

// =============================================================================
// ViewConfigEnvelope — Structural, not versioned
// =============================================================================

/// Structural envelope wrapping persisted configuration data.
///
/// `data` is `serde_json::Value` — typed deserialization happens AFTER normalization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewConfigEnvelope {
    pub schema: String,
    pub widget_type: String,
    pub view_id: String,
    pub structure_hash: String,
    pub data: serde_json::Value,
}

// =============================================================================
// ViewConfigSchema trait — Widget-specific normalization
// =============================================================================

/// Widget-specific normalization trait.
///
/// Each widget type implements its own normalization logic.
pub trait ViewConfigSchema {
    const WIDGET_TYPE: &'static str;
    fn structure_hash(&self) -> String;
    fn normalize(raw: serde_json::Value, live_model: &Self) -> serde_json::Value;
}

// =============================================================================
// ViewConfigStore trait — Abstract backend
// =============================================================================

/// Abstract persistence backend.
///
/// Implementations may use filesystem, database, in-memory, etc.
pub trait ViewConfigStore {
    fn load(&self, key: &ViewConfigKey) -> Option<serde_json::Value>;
    fn save(&self, key: &ViewConfigKey, value: &serde_json::Value);
    fn clear(&self, key: &ViewConfigKey);
}

// =============================================================================
// compute_structure_hash — Topology fingerprint
// =============================================================================

/// Compute a SHA-256 structure hash from a column model.
///
/// Hash covers: per-column (ColumnId raw, scroll_domain, preferred_width).
/// Used to detect structural changes between save and load.
pub fn compute_structure_hash(model: &ColumnModel) -> String {
    let mut hasher = Sha256::new();
    for col in model.columns() {
        hasher.update(col.id.raw().to_le_bytes());
        hasher.update(format!("{:?}", col.scroll_domain).as_bytes());
        hasher.update(col.preferred_width.to_le_bytes());
    }
    let result = hasher.finalize();
    format!("sha256:{:x}", result)
}

// =============================================================================
// reconcile_json — Structural reconciliation on raw JSON
// =============================================================================

/// Reconcile saved JSON data against the live column model.
///
/// Operates on JSON shape FIRST, not typed state:
/// 1. Extract column_order array from JSON
/// 2. Drop unknown column IDs (not in live model)
/// 3. Preserve known ones in their saved order
/// 4. Append new live columns (in live model but not saved) at end
/// 5. Filter preferred widths and hidden/pinned sets to valid IDs only
/// 6. Ensure at least one visible column
pub fn reconcile_json(raw_data: &serde_json::Value, live_model: &ColumnModel) -> serde_json::Value {
    let live_ids: std::collections::HashSet<u64> =
        live_model.columns().iter().map(|c| c.id.raw()).collect();

    // Extract column_order or default to empty.
    let saved_order: Vec<u64> = raw_data
        .get("column_order")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    // Keep known IDs in saved order.
    let mut reconciled_order: Vec<u64> = saved_order
        .into_iter()
        .filter(|id| live_ids.contains(id))
        .collect();

    // Append new live columns not in saved order.
    let saved_set: std::collections::HashSet<u64> = reconciled_order.iter().copied().collect();
    for col in live_model.columns() {
        if !saved_set.contains(&col.id.raw()) {
            reconciled_order.push(col.id.raw());
        }
    }

    // Filter hidden/pinned sets to valid IDs.
    let filter_set = |key: &str| -> Vec<u64> {
        raw_data
            .get(key)
            .and_then(|v| serde_json::from_value::<Vec<u64>>(v.clone()).ok())
            .unwrap_or_default()
            .into_iter()
            .filter(|id| live_ids.contains(id))
            .collect()
    };

    let hidden: Vec<u64> = filter_set("hidden");
    let pinned_left: Vec<u64> = filter_set("pinned_left");
    let pinned_right: Vec<u64> = filter_set("pinned_right");
    let mut preferred_widths = serde_json::Map::new();
    if let Some(saved_widths) = raw_data
        .get("preferred_widths")
        .and_then(|value| value.as_object())
    {
        for (raw_id, value) in saved_widths {
            let Some(id) = raw_id.parse::<u64>().ok() else {
                continue;
            };
            let Some(width) = value.as_f64() else {
                continue;
            };
            if live_ids.contains(&id) && width.is_finite() && width > 0.0 {
                preferred_widths.insert(raw_id.clone(), value.clone());
            }
        }
    }
    let sort = raw_data
        .get("sort")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    // Ensure at least one visible column.
    let hidden_set: std::collections::HashSet<u64> = hidden.iter().copied().collect();
    let visible_count = reconciled_order
        .iter()
        .filter(|id| !hidden_set.contains(id))
        .count();
    let final_hidden = if visible_count == 0 {
        // Drop all hidden to make everything visible.
        Vec::new()
    } else {
        hidden
    };

    serde_json::json!({
        "column_order": reconciled_order,
        "preferred_widths": preferred_widths,
        "hidden": final_hidden,
        "pinned_left": pinned_left,
        "pinned_right": pinned_right,
        "sort": sort,
    })
}

// =============================================================================
// normalize_json — Canonical form enforcement
// =============================================================================

/// Normalize raw JSON into canonical ColumnConfigurationState shape.
///
/// Tolerates extra unknown fields. Injects missing fields with defaults.
pub fn normalize_json(raw: &serde_json::Value) -> serde_json::Value {
    let column_order = raw
        .get("column_order")
        .cloned()
        .unwrap_or(serde_json::json!([]));
    let preferred_widths = raw
        .get("preferred_widths")
        .cloned()
        .unwrap_or(serde_json::json!({}));
    let hidden = raw.get("hidden").cloned().unwrap_or(serde_json::json!([]));
    let pinned_left = raw
        .get("pinned_left")
        .cloned()
        .unwrap_or(serde_json::json!([]));
    let pinned_right = raw
        .get("pinned_right")
        .cloned()
        .unwrap_or(serde_json::json!([]));
    let sort = raw.get("sort").cloned().unwrap_or(serde_json::Value::Null);

    serde_json::json!({
        "column_order": column_order,
        "preferred_widths": preferred_widths,
        "hidden": hidden,
        "pinned_left": pinned_left,
        "pinned_right": pinned_right,
        "sort": sort,
    })
}

// =============================================================================
// Substrate-level persistence free functions
// =============================================================================

/// Save current column configuration to the store.
pub fn save_column_config(model: &ColumnModel, key: &ViewConfigKey, store: &dyn ViewConfigStore) {
    if !key.is_persistence_enabled() {
        return;
    }

    let state = ColumnConfigurationState::from_model(model);
    let mut data_json = serde_json::to_value(&state).unwrap_or(serde_json::json!({}));
    if let Some(existing) = store.load(key)
        && let Ok(existing) = serde_json::from_value::<ViewConfigEnvelope>(existing)
        && let Some(sort) = existing.data.get("sort")
    {
        data_json["sort"] = sort.clone();
    }
    let canonical = normalize_json(&data_json);

    let envelope = ViewConfigEnvelope {
        schema: "halcyon.view.config".to_string(),
        widget_type: key.widget_type.clone(),
        view_id: key.view_id.clone(),
        structure_hash: compute_structure_hash(model),
        data: canonical,
    };

    let envelope_json = serde_json::to_value(&envelope).unwrap_or(serde_json::json!({}));
    store.save(key, &envelope_json);
}

/// Save a requested sort preference without treating it as applied provider truth.
pub fn save_sort_preference(
    model: &ColumnModel,
    spec: &SortSpec,
    key: &ViewConfigKey,
    store: &dyn ViewConfigStore,
) {
    if !key.is_persistence_enabled() {
        return;
    }

    let state = ColumnConfigurationState::from_model(model);
    let mut data = serde_json::to_value(&state).unwrap_or(serde_json::json!({}));
    data["sort"] = serde_json::to_value(spec).unwrap_or(serde_json::Value::Null);
    let envelope = ViewConfigEnvelope {
        schema: "halcyon.view.config".to_string(),
        widget_type: key.widget_type.clone(),
        view_id: key.view_id.clone(),
        structure_hash: compute_structure_hash(model),
        data: normalize_json(&data),
    };
    if let Ok(value) = serde_json::to_value(envelope) {
        store.save(key, &value);
    }
}

/// Decode a persisted requested sort preference for provider revalidation.
pub fn sort_preference_from_raw_json(
    key: &ViewConfigKey,
    raw: &serde_json::Value,
) -> Option<SortSpec> {
    let envelope = serde_json::from_value::<ViewConfigEnvelope>(raw.clone()).ok()?;
    if envelope.schema != "halcyon.view.config"
        || envelope.widget_type != key.widget_type
        || envelope.view_id != key.view_id
    {
        return None;
    }
    serde_json::from_value(envelope.data.get("sort")?.clone()).ok()
}

/// Remove only the saved sort preference while preserving column presentation.
pub fn clear_sort_preference(
    model: &ColumnModel,
    key: &ViewConfigKey,
    store: &dyn ViewConfigStore,
) {
    if !key.is_persistence_enabled() {
        return;
    }
    let state = ColumnConfigurationState::from_model(model);
    let mut data = serde_json::to_value(&state).unwrap_or(serde_json::json!({}));
    data["sort"] = serde_json::Value::Null;
    let envelope = ViewConfigEnvelope {
        schema: "halcyon.view.config".to_string(),
        widget_type: key.widget_type.clone(),
        view_id: key.view_id.clone(),
        structure_hash: compute_structure_hash(model),
        data: normalize_json(&data),
    };
    if let Ok(value) = serde_json::to_value(envelope) {
        store.save(key, &value);
    }
}

/// Load and apply persisted column configuration.
///
/// Returns true if configuration was loaded and applied.
pub fn load_and_apply_column_config(
    substrate: &mut super::TabularSubstrate,
    key: &ViewConfigKey,
    store: &dyn ViewConfigStore,
) -> bool {
    if !key.is_persistence_enabled() {
        return false;
    }

    let Some(raw) = store.load(key) else {
        return false;
    };

    // Deserialize envelope.
    let Ok(envelope) = serde_json::from_value::<ViewConfigEnvelope>(raw) else {
        return false;
    };

    // Validate schema.
    if envelope.schema != "halcyon.view.config" {
        return false;
    }

    // Validate widget_type.
    if envelope.widget_type != key.widget_type {
        return false;
    }

    // Check structure hash — reconcile if mismatch.
    let live_hash = compute_structure_hash(substrate.column_model());
    let data = if envelope.structure_hash != live_hash {
        reconcile_json(&envelope.data, substrate.column_model())
    } else {
        envelope.data
    };

    // Normalize.
    let canonical = normalize_json(&data);

    // Deserialize typed state.
    let Ok(config) = serde_json::from_value::<ColumnConfigurationState>(canonical) else {
        return false;
    };

    // Apply.
    substrate.apply_column_configuration(&config)
}

/// Apply raw config JSON (already loaded from store) to substrate.
///
/// Useful when the store has been read separately to avoid borrow conflicts.
/// Returns true if configuration was applied.
pub fn apply_raw_config_json(
    substrate: &mut super::TabularSubstrate,
    key: &ViewConfigKey,
    raw: serde_json::Value,
) -> bool {
    let Some(config) = column_config_from_raw_json(key, raw, substrate.column_model()) else {
        return false;
    };
    substrate.apply_column_configuration(&config)
}

/// Decode raw config JSON against the live column model.
pub fn column_config_from_raw_json(
    key: &ViewConfigKey,
    raw: serde_json::Value,
    model: &ColumnModel,
) -> Option<ColumnConfigurationState> {
    let Ok(envelope) = serde_json::from_value::<ViewConfigEnvelope>(raw) else {
        return None;
    };

    if envelope.schema != "halcyon.view.config" {
        return None;
    }
    if envelope.widget_type != key.widget_type {
        return None;
    }

    let live_hash = compute_structure_hash(model);
    let data = if envelope.structure_hash != live_hash {
        reconcile_json(&envelope.data, model)
    } else {
        envelope.data
    };

    let canonical = normalize_json(&data);
    let Ok(config) = serde_json::from_value::<ColumnConfigurationState>(canonical) else {
        return None;
    };
    Some(config)
}

/// Reset column configuration to default and clear persisted state.
pub fn reset_column_config(
    substrate: &mut super::TabularSubstrate,
    default_config: &ColumnConfigurationState,
    key: &ViewConfigKey,
    store: &dyn ViewConfigStore,
) {
    substrate.apply_column_configuration(default_config);
    if key.is_persistence_enabled() {
        store.clear(key);
    }
}
