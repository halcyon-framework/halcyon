**SPEC-HAL-022: Halcyon — Settings, Persistence & Configuration Layer**
**Author:** JC
**Date:** 2025-11-17
**Status:** Draft
**Scope:** App settings, UI layout persistence, widget configuration storage, profile system, JSON/TOML schema, autosave/restore.
**Purpose:** Make Halcyon apps *stateful*, persistent, and user-friendly without turning the system into a bloated “mega framework.”

This SPEC defines the **entire persistence layer** Halcyon needs so that:

* your trading workspace
* chart layout
* ladder configuration
* window positions
* theme
* hotkeys
* app settings
  all survive between launches.

---

# 1. Philosophy

Halcyon must support **long-running**, **stateful**, **real-world apps**.

Persistence must be:

* **Explicit** (nothing auto-magical)
* **Safe** (no corruption)
* **Minimal** (only what apps choose to persist)
* **Stable** (versioned schemas)
* **Serializable** (JSON/TOML)
* **Zero global singletons**

No hidden “global SettingsRegistry,” no magic sync.
We explicitly define *what is saved*, *how*, *when*, and *where*.

---

# 2. High-Level Architecture

```
+-------------------------+
|     AppState (runtime) |
+------------+------------+
             | read/write
             v
+-------------------------+
| SettingsManager        |
| - profiles             |
| - versions             |
| - serialization       |
+------------+------------+
             | disk I/O
             v
+-------------------------+
| config/halcyon/…       |
| - settings.json        |
| - profiles/…           |
| - layouts/…            |
+-------------------------+
```

Everything revolves around **SettingsManager**.

---

# 3. What Should Be Persisted?

For trading-grade apps:

### App-Level Configuration

* theme preference (dark/light/custom)
* user profile name
* chart defaults (candle spacing, smoothing)
* hotkeys
* notifications
* network endpoints
* trading parameters

### UI Layout

* window positions
* panel sizes
* docking state
* split ratios
* collapsed/expanded panes
* last open workspace

### Widget Settings

Each widget can define a struct:

```rust
#[derive(Serialize, Deserialize)]
struct WidgetConfig {
    some_flag: bool,
    column_widths: Vec<f32>,
    sort_order: SortKey,
}
```

Widgets can load/save this via `SettingsManager`.

### Profiles

Multiple independent user environments:

```
profiles/
    default/
    scalping/
    swing/
    dev/
```

Each maintains:

* app settings
* workspace layout
* widget configs

Switching profile = full environment swap.

---

# 4. SettingsManager

Central component owned by Halcyon runtime.

```rust
pub struct SettingsManager {
    base_dir: PathBuf,
    profile: String,

    // current in-memory loaded settings
    app_settings: serde_json::Value,
    layout_settings: serde_json::Value,
    widget_settings: HashMap<WidgetTypeId, serde_json::Value>,
}
```

### Core Methods

```rust
impl SettingsManager {
    pub fn load_profile(&mut self, name: &str) -> Result<()>;

    pub fn save_profile(&self) -> Result<()>;

    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T>;

    pub fn set<T: Serialize>(&mut self, key: &str, value: &T);

    pub fn save_widget<T: Serialize>(
        &mut self,
        widget_id: WidgetId,
        value: &T
    );

    pub fn load_widget<T: DeserializeOwned>(
        &self,
        widget_id: WidgetId
    ) -> Option<T>;
}
```

Widgets explicitly load their configs in constructor and save when changed.

---

# 5. File Layout on Disk

Inside user’s OS config directory:

### Linux

`~/.config/halcyon/`

### Windows

`%AppData%\Halcyon\`

### macOS

`~/Library/Application Support/Halcyon/`

### Directory Structure

```
halcyon/
    settings.json         (global)
    profiles/
        default/
            app.json
            layout.json
            widgets/
                ChartWidget.json
                LadderWidget.json
                …
        scalping/
        swing/
        …
```

No binary blobs unless explicitly needed (images, custom themes).

---

# 6. Settings Format

**JSON** as default.
**TOML** optional.

Schema versioning and migration supported.

Top-level:

```json
{
  "version": 1,
  "theme": "dark",
  "last_workspace": "main",
  "keybinds": {
    "enter_order": "Ctrl+E",
    "flatten": "Shift+F"
  }
}
```

Layouts:

```json
{
  "windows": [
    {"id": "main", "x": 230, "y": 140, "w": 1400, "h": 900}
  ],
  "dock": {
    "left": ["MarketWatch", "Positions"],
    "right": ["Chart1", "Chart2"]
  }
}
```

Widgets:

```json
{
  "column_widths": [120, 80, 90],
  "show_bid_ask": true,
  "sort": "PriceDesc"
}
```

---

# 7. Autosave

### When?

* On profile switch
* On clean exit
* After layout changes (debounced)
* After widget config change (direct or debounced)

### Debounce Strategy

```rust
save_after(300ms)
```

Every change schedules a save 300ms later.
If another change happens inside that window, timer resets.

---

# 8. Profile Switching

### Benefits:

* “scalping” profile: aggressive ladder layout
* “analysis” profile: big charts, multiple monitors
* “dev” profile: debug UI

### Model:

```rust
settings_manager.load_profile("scalping");
```

Triggers:

1. save old profile
2. clear widget tree
3. reload app state
4. rebuild layout
5. render new workspace

---

# 9. Versioning & Migration

Every settings file has:

```json
"version": 1
```

When loading:

* if version == runtime version → OK
* if version < runtime → run migration chain
* if version > runtime → refuse or downgrade

Migration example:

```rust
fn migrate_v1_to_v2(json: &mut Value) {
    json["new_field"] = json!("default");
}
```

---

# 10. Integration With Halcyon Runtime

### AppRunner owns:

```rust
pub struct HalcyonRuntime {
    settings: SettingsManager,
    timeline: Timeline,
    widget_tree: WidgetTree,
    platform: PlatformBackend,
    renderer: RendererWgpu,
    app: A,
}
```

On startup:

```
runtime.settings.load_profile("default")
app.init_from_settings(&runtime.settings)
runtime.restore_layout()
```

On exit:

```
runtime.settings.save_profile()
```

---

# 11. Widget & Settings Interaction

Each widget can:

### Load

```rust
if let Some(cfg) = settings.load_widget::<MyConfig>(self.id) {
    self.state.apply_config(cfg);
}
```

### Save

```rust
settings.save_widget(self.id, &self.state.to_config());
```

Widgets decide *what* to persist.
Halcyon just stores the blob.

---

# 12. Trading-Specific Requirements

Halcyon must persist:

### Chart:

* timeframe
* zoom level
* viewport
* overlays
* indicators
* color scheme

### Ladder:

* visible levels
* column widths
* colors
* hotkey modes
* bracket orders config

### Panels:

* pinned states
* split ratios
* tab positions

### App:

* last profile
* last workspace
* risk settings
* routing settings

Everything must be instantly restorable.

---

# 13. Performance Requirements

* ZERO allocations in widget code for persistence
* Disk I/O async or off main thread
* JSON encode/decode <1ms for typical size
* Autosave does not stall frame loop
* Must work with multiple profiles (1–10 MB total data)

---

# 14. Implementation Checklist (Claude Code)

### SettingsManager

* [ ] Implement load/save for profiles
* [ ] Implement versioning
* [ ] Implement widget config load/save
* [ ] Use serde_json

### Layout Persistence

* [ ] Record window positions
* [ ] Record panel sizes
* [ ] Serialize to layout.json

### Widget Integration

* [ ] Add widget-type identifier
* [ ] Implement to_config / apply_config pattern

### App Integration

* [ ] App::init_from_settings() in main app trait
* [ ] Add save on exit
* [ ] Add profile switching API

---

# 15. Result

After implementing SPEC-HAL-022:

* Halcyon becomes a *real* application framework
* Trading dashboards restore perfectly
* Multi-profile workflows exist
* UI customizations persist
* Chart & ladder widgets reload exactly as used last session
* Layouts behave like professional terminals

This is the baseline that every serious application — especially a trading system — absolutely requires.
