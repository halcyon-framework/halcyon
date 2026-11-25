# SPEC-HAL-320: Halcyon MCP Bridge — Remote Agent Orchestration

**Author:** JC
**Status:** Draft
**Depends on:** SPEC-HAL-317 (Agent Infrastructure), SPEC-HAL-318 (Tower), SPEC-HAL-319 (Runtime UI Composition)

---

## 1. Purpose

Enable external AI agents (Claude via MCP, other LLMs) to observe and control Halcyon's runtime UI composition through a standardized interface. The MCP Bridge acts as a coordinator between:

- **Claude (remote)** — architectural authority, gives high-level instructions
- **Tower (local LLM)** — execution agent, translates instructions to composition commands
- **Canvas (Halcyon)** — the running UI application
- **LM Studio** — local model server, manages loaded models

```
Claude ←→ MCP Bridge ←→ Tower (local LLM) ←→ Canvas (Halcyon)
              ↕                                    ↕
        LM Studio API                    WebSocket :9317
```

### 1.1 Authority Boundary

The MCP Bridge is a Wireframe/Tower composition surface. It connects
only to the SPEC-HAL-317 `Full` Agent server profile and intentionally
exposes SPEC-HAL-319 composition, snapshot, and window tools. It is
not the control surface for inspecting a production-shaped consumer
application.

Consumer-app development verification uses the separate
`DevelopmentUiControl` profile and the typed `AgentClient` or
`agent_control` binary. That path exposes only filtered, bounded
`FindWidgets`, exact `WidgetState`/`FocusState`, and ordinary fused
input. It does not expose full tree/visible-widget snapshots,
composition, Tower orchestration, or window mutation. The two
profiles MUST NOT be treated as interchangeable.

## 2. MCP Bridge Tools

The MCP Bridge exposes these tools to Claude:

### 2.1 UI Observation

**`halcyon_view_tree`**
Returns the compact tree representation of the current canvas UI (~200 tokens).
Connects to canvas WebSocket, sends compact Snapshot, returns result.

**`halcyon_view_widget`**  
Returns detailed info for a specific widget by ID (bounds, hint, type, config).

### 2.2 Direct Composition

**`halcyon_add_widget`**
Add a widget to the canvas. Parameters: parent_id, widget_type, config JSON.

**`halcyon_configure_widget`**
Configure an existing widget. Parameters: widget_id, config JSON (spacing, grow, text, style, value).

**`halcyon_remove_widget`**
Remove a widget by ID.

### 2.3 Agent Orchestration

**`halcyon_instruct_agent`**
Send a natural language instruction to Tower. Tower translates to composition commands and executes. Returns Tower's response (what it did, success/failure).

**`halcyon_agent_status`**
Returns: Tower connected (bool), model loaded (name), canvas connected (bool), widget count.

### 2.4 Model Management

**`halcyon_list_models`**
Query LM Studio API for available and loaded models.

**`halcyon_load_model`**
Tell LM Studio to load a specific model by name.

## 3. Tower Control Channel

Tower needs a control channel for receiving instructions from the MCP Bridge. Currently Tower only connects to the canvas WebSocket and runs its own LLM loop. The control channel adds an external instruction input.

### 3.1 Protocol

TCP listener on port **9318**. Line-delimited JSON protocol:

**Instruction (MCP Bridge → Tower):**
```json
{"type":"instruct","id":"req-1","text":"Set grow:1.0 on all labeled field columns in row containers"}
```

**Status query (MCP Bridge → Tower):**
```json
{"type":"status","id":"req-2"}
```

**Response (Tower → MCP Bridge):**
```json
{"type":"response","id":"req-1","status":"ok","actions_taken":3,"detail":"Set grow on widgets 35, 38, 41"}
```

```json
{"type":"status_response","id":"req-2","model":"gpt-oss-20b","connected":true,"idle":true}
```

### 3.2 Instruction Processing

When Tower receives an instruction:
1. Query canvas for compact tree (context)
2. Build LLM prompt: system prompt (composition protocol) + compact tree + instruction
3. LLM generates composition commands
4. Tower sends commands to canvas
5. Tower queries updated tree
6. Tower reports result back to MCP Bridge

### 3.3 Autonomous vs Directed Mode

- **Directed mode** (default): Tower waits for instructions from the control channel. Executes one instruction at a time, reports result.
- **Autonomous mode**: Tower receives a goal ("make this look like a professional settings screen") and iterates until convergence or timeout.

## 4. MCP Bridge Implementation

### 4.1 Binary

New example or binary: `examples/mcp_bridge.rs` (or `src/bin/mcp_bridge.rs`).

### 4.2 Dependencies

- `tungstenite` — WebSocket client to canvas (:9317)
- `reqwest` or `ureq` — HTTP client to LM Studio API (:1234)
- `tokio` — async runtime for MCP server + TCP connections
- `serde_json` — JSON protocol

### 4.3 MCP Protocol

The bridge speaks MCP over **stdio** (simplest for local use with Claude Code) or **HTTP+SSE** (for Claude.ai web interface).

For initial implementation: **stdio** mode. Claude Code can connect directly. For Claude.ai web: needs a tunnel (ngrok or similar) or a hosted proxy.

### 4.4 Connection Lifecycle

- MCP Bridge starts → connects to canvas WebSocket → connects to Tower TCP (:9318) → connects to LM Studio HTTP (:1234)
- Claude connects to MCP Bridge → tools become available
- Claude disconnects → MCP Bridge stays running, Tower continues
- Tower disconnects → MCP Bridge reports "agent offline" on status queries
- Canvas restarts → MCP Bridge reconnects automatically

---

# TASK: Tower Control Channel (Phase 1)

## Pre-Flight

- RFC: SPEC-HAL-318, SPEC-HAL-320, read: YES
- Policy: N/A
- Interaction added: NO
- Forbidden behaviors checked: N/A
- Existing infrastructure: Tower, TowerHandle, TowerConfig
- Reinvention risk: none

## Overview

Add a TCP control channel to Tower so external systems (MCP Bridge, designer CLI) can send instructions and query status.

## Implementation

### File: `src/agent/tower.rs`

**Edit 1** — Add `control_port` to `TowerConfig`:

```rust
pub struct TowerConfig {
    pub endpoint: String,
    pub model: String,
    pub system_prompt: String,
    pub control_port: Option<u16>,  // NEW: TCP control channel port (default: 9318)
}
```

**Edit 2** — In the Tower event loop (the main `tokio::spawn` task), add a TCP listener alongside the existing LLM loop:

```rust
// Spawn control channel listener
if let Some(port) = config.control_port {
    let control_listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    eprintln!("[Tower] Control channel listening on 127.0.0.1:{}", port);
    
    tokio::spawn(async move {
        loop {
            if let Ok((stream, addr)) = control_listener.accept().await {
                eprintln!("[Tower] Control client connected: {}", addr);
                // Handle control connection
                handle_control_connection(stream, /* shared state */).await;
            }
        }
    });
}
```

**Edit 3** — Implement `handle_control_connection`:

```rust
async fn handle_control_connection(
    stream: TcpStream,
    // Shared: canvas WebSocket sender, LLM client, tower state
) {
    let (reader, writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    
    while let Some(Ok(line)) = lines.next_line().await {
        let request: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                // Send error response
                continue;
            }
        };
        
        let msg_type = request.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let msg_id = request.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        
        match msg_type {
            "status" => {
                // Return current Tower state
                let response = json!({
                    "type": "status_response",
                    "id": msg_id,
                    "model": config.model,
                    "connected": true,
                    "idle": !is_processing.load(Ordering::Relaxed),
                });
                // Write response
            }
            "instruct" => {
                let instruction = request.get("text").and_then(|v| v.as_str()).unwrap_or("");
                // 1. Get compact tree from canvas
                // 2. Build LLM prompt with instruction + tree context
                // 3. Send to LLM
                // 4. Parse LLM response as composition commands
                // 5. Send commands to canvas
                // 6. Report result
            }
            _ => {
                // Unknown message type
            }
        }
    }
}
```

CC: the Tower's main loop already has:
- Canvas WebSocket connection (for sending actions and reading events)
- LLM client (for generating responses)
- Query capabilities (visible_widgets, focus_state)

The control channel handler needs access to the same canvas connection and LLM client. Use `Arc<Mutex<>>` or channels to share access between the main Tower loop and the control channel handler.

The `instruct` handler flow:
1. Send `{"Snapshot":{"name":"ctx","format":"compact"}}` to canvas WebSocket
2. Read compact tree response (filter non-composition events)
3. Build prompt: `"Current UI:\n{compact_tree}\n\nInstruction: {instruction}\n\nRespond with JSON composition commands."`
4. Send to LLM via the existing LLM client
5. Parse LLM response — extract composition commands (AddWidget, ConfigureWidget, etc.)
6. Send each command to canvas WebSocket
7. Collect results
8. Send response back to control channel: `{"type":"response","id":"...","status":"ok","actions_taken":N,"detail":"..."}`

### File: `examples/tower_embedded_demo.rs` (or whichever example uses Tower)

Update to pass `control_port: Some(9318)` in TowerConfig.

### File: `examples/canvas.rs`

No changes needed — canvas already accepts WebSocket connections from any client.

## Verification

1. `grip:check_health` — 0 errors
2. Start canvas: `cargo run --example canvas`
3. Start compose_settings to seed UI
4. Start tower_embedded_demo (with control channel on :9318)
5. Connect via netcat: `echo '{"type":"status","id":"1"}' | nc 127.0.0.1 9318`
6. Get status response with model name and connected=true
7. Send instruction: `echo '{"type":"instruct","id":"2","text":"Set grow 1.0 on widget 35"}' | nc 127.0.0.1 9318`
8. Get response with actions_taken and detail
9. Canvas shows the change
