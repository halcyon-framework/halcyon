---
doc-id: halcyon-guide-agent
class: first-class
authority: explanatory-non-binding
halcyon-revision: b3be05fcbd6bbb11d6750520991a1bacd3c1ccde
verified-at: 2026-07-24
governing-specs:
  - SPEC-HAL-317
  - SPEC-HAL-318
---

# Agent and Remote Control

How an external agent observes and drives a running Halcyon application.
Revision `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde`; two-axis tags per
SPEC-HAL-309 §7.

## 1. The Stack

```text
Halcyon engine (frame loop)
  ├─ emits AgentEvent (bounded channel, fire-and-forget)     src/core/agent.rs:1-16
  ├─ drains AgentAction during Phase 1 with OS events        src/core/app.rs:7121-7123 (AppConfig doc)
  └─ Agent WebSocket server (SPEC-HAL-317)                   src/core/agent_server.rs:1-5
       one dedicated sync thread · JSON over WebSocket · no async runtime
       INV-317-3: never blocks the render loop
  ← AgentClient (typed, bounded dev client)                  src/core/agent_client.rs:1-30
  ← Tower orchestrator (SPEC-HAL-318, optional)              src/agent/mod.rs:1-14
```

## 2. Wiring It Up (AppConfig seam)

All agent integration enters through `AppConfig`
[implementation: implemented — `src/core/app.rs:7108-7134`]:

- `agent_tx: Option<AgentEventSender>` — "If set, UI events are emitted
  for agent observation. None = no agent events emitted."
- `agent_action_rx: Option<AgentActionReceiver>` — "If set, the engine
  drains agent actions during Phase 1 alongside OS events."
- `agent_server_port: Option<u16>` — "None = no server."
- `agent_server_profile: AgentServerProfile` — capability boundary
  "enforced before external actions enter the engine."
- `tower_config: Option<TowerConfig>` — "If set, Tower spawns
  automatically during App::run."

Channel construction:
`AppConfig::with_agent_channel() -> (config, AgentEventReceiver,
AgentActionSender)` [implementation: implemented —
`src/core/app.rs:7178-7190`]. Defaults: all agent fields off,
`AgentServerProfile::Full` [implementation: implemented —
`src/core/app.rs:7136-7151`].

## 3. Observation — AgentEvent

`AgentEvent { timestamp, kind }` on a bounded fire-and-forget channel:
"if no agent is listening, events are dropped"; the channel never blocks
the render loop (INV-317-3)
[implementation: implemented — `src/core/agent.rs:1-25`].
`AgentEventKind` — complete variant set at this revision
[implementation: implemented — `src/core/agent.rs:29-90`]:
`FocusChanged { old, new }`, `PointerDown { widget_id, button, x, y }`,
`KeyDown { widget_id, key }`, `TextInput { widget_id, text }`,
`FrameRendered { frame_number, duration_us }`,
`Custom { tag, payload }`, `QueryResult { query, data }`,
`CompositionResult { action, success, widget_id, message }`,
`BlueprintSnapshot { name, data }`, `CompositionDiff { diff }`
(SPEC-HAL-319), `AssemblyResult { action, success, assembly_id, ids,
message }` (SPEC-HAL-321). Bounded-query constants beside it
(`:93-105`): channel capacity 10 000, find limit 32 default / 64 max,
≤64 child IDs and ≤4 096 UTF-8 bytes per bounded widget-state record.

## 4. Control — AgentAction and Capability Profiles

Actions arrive over the WebSocket server and are authorized **before**
entering the engine channel
[implementation: implemented — `src/core/agent_server.rs:41-60`]:

- `AgentServerProfile::Full` — "Unrestricted Wireframe/Tower profile
  retained for Halcyon composition tools." Authorization is a passthrough.
- `AgentServerProfile::DevelopmentUiControl` — "Debug-only consumer-app
  inspection and ordinary fused-input control." The authorizer admits
  `Click`, `DoubleClick`, `Focus`, finite-delta `Scroll`, and function
  keys only in F1-F12; malformed variants are rejected with typed
  rejections (`AgentActionRejection`)
  [implementation: implemented — `src/core/agent_server.rs:25-60`].

Wire-level bounds are explicit constants: filter strings ≤ 256 bytes,
text input ≤ 16 384 bytes (`MAX_AGENT_FILTER_BYTES`,
`MAX_AGENT_TEXT_INPUT_BYTES`, `src/core/agent_server.rs:20-21`).

## 5. Bounded Querying

The server speaks `AgentQuery` with explicit result bounds —
`DEFAULT_AGENT_FIND_LIMIT` / `MAX_AGENT_FIND_LIMIT` are part of the
channel vocabulary [implementation: implemented — imports at
`src/core/agent_server.rs:7-10`; types in `src/core/agent.rs`].
`AgentClient` is the typed consumer: "bounded external Agent client for
development verification" with response timeouts and typed
rejection/timeout errors [implementation: implemented —
`src/core/agent_client.rs:1-30`].

Examples exercising this path: `agent`, `agent_query`,
`agent_test_client`, `agent_lm_bridge`, `tower_embedded`
(`examples-index.md` §Agent).

## 6. Tower — Orchestration (SPEC-HAL-318)

Tower is the optional orchestration daemon above the raw channel:
"System module for autonomous UI exploration and testing. Bridges a
running Halcyon app to a local LLM via the agent WebSocket, supervised by
an external architect via MCP"
[implementation: implemented — `src/agent/mod.rs:1-14`; modules:
`llm_client`, `mock_data`, `orchestrator`, `playground_loop`, `tower`,
`widget_factory`; exports `Tower`, `TowerConfig`, `TowerMode`].

SPEC-HAL-318 is `Draft` [authority: draft-intent — SPEC-HAL-318:6]; it
sketches configuration, spawning, objectives, findings, LLM babysitting,
an MCP interface, and AppConfig integration (SPEC-HAL-318 §5-§8). Treat
its API details as intent; the implemented surface is the `src/agent/`
module set plus `tower_config` auto-spawn (§2).

## 7. Boundaries

- The agent path is observation + fused-input-level control; actions enter
  the engine through the same Phase-1 intake as OS events (§2) — no
  bypass of routing, focus, capture, or the deferred-mutation law
  [authority: binding — SPEC-HAL-002 §3.7/§4.2; mechanism in
  `runtime-and-frame-lifecycle.md` §4].
- Profile enforcement happens server-side before the engine channel (§4);
  application code selects the profile once in `AppConfig`.
- The server thread is sync and isolated (INV-317-3); do not add async
  runtimes to it [implementation: implemented-declaration —
  `src/core/agent_server.rs:1-5`].
- Composition-class actions (widget creation, structural mutation) exist
  for the Full/Wireframe profile and land in the frame's pending-
  composition stage (SPEC-HAL-319; `runtime-and-frame-lifecycle.md` §4
  step 12) — they are not part of the development-control profile (§4).
