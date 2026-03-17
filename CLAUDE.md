# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**SendIT** — a native desktop GUI application for drag-and-drop file transfer over Zenoh peer-to-peer networks. Built in Rust with egui/eframe.

## Build & Run

```bash
cargo build                              # Debug build
cargo build --release                    # Release (LTO, stripped, panic=abort)
cargo run                                # Run debug
cargo run --release                      # Run release
RUST_LOG=send_it=debug cargo run         # Run with debug logging
```

No test suite. No CI/CD. Docker build available via `Dockerfile` (rust:1.93-alpine).

## Module Structure

```
src/
├── main.rs              # Entry point only (~60 lines)
├── app.rs               # SendItApp struct, construction, theme helpers, eframe::App impl
├── events.rs            # Event processing, dedup, browse tree updates, message storage
├── colors.rs            # SendItColors light/dark palette constants
├── types.rs             # Shared types, enums, constants (ZenohCommand, ZenohEvent, etc.)
├── transfer.rs          # Chunk reassembly, export to file, chunk info scanning
├── zenoh_worker.rs      # Async Tokio worker: connections, publish dispatch, subscribe, query
└── ui/
    ├── mod.rs           # UI module declarations
    ├── drop_zone.rs     # DropZoneUI trait: drag-and-drop file sending (central panel)
    ├── settings.rs      # SettingsUI trait: collapsible settings panel (connection, subs, query, memory)
    ├── topic_tree.rs    # TopicTreeUI trait: tree panel, topic details, tree node rendering
    ├── publish.rs       # PublishUI trait: publish tab + queryable section (accessible via settings)
    ├── query.rs         # QueryUI trait: query tab + results display (accessible via settings)
    ├── messages.rs      # MessagesUI trait: message log with filtering
    └── help.rs          # HelpUI trait: help/usage tab
```

### Module Pattern
UI rendering uses **trait-based decomposition**: each `ui/*.rs` file defines a trait (e.g. `TopicTreeUI`) that `SendItApp` implements. The traits must be imported where their methods are called:
- `app.rs` imports `TopicTreeUI` (for `show_tree_panel`/`show_topic_details`) and `DropZoneUI` (for `show_drop_zone`)
- `topic_tree.rs` imports `SettingsUI` (for collapsible settings above tree) and `MessagesUI` (for "All Messages" fallback)

Event processing methods (`process_events`, `is_duplicate`, `add_message_to_browse_tree`, `add_message_with_limits`) are direct `impl SendItApp` methods in `events.rs`.

All struct fields are `pub(crate)` to allow access from sibling modules.

### UI Layout
- **Toolbar** (top): "SendIT" title, connection status dot + peer count, theme toggle
- **Left panel**: Gear icon → collapsible settings, search filter, topic tree
- **Central panel**: Drop zone (idle/hover/sending/success/error states)
- **Right panel** (slide-out): Topic details when a topic is selected (export, chunk progress, payload preview)

### Connection Flow
Auto-connects on launch: peer mode port 7447 with multicast discovery. Connection failure shows an overlay on the drop zone. Monitor session (background `**` subscription) starts lazily when first data arrives. Auto-subscribes to `**` on connect.

## Architecture

### Dual-Thread Model
- **GUI thread** (main): egui/eframe rendering + user interaction + state management
- **Worker thread**: Tokio async runtime for all Zenoh network operations
- **Buffer thread**: Batches messages from worker → GUI (50 msgs OR 16ms flush interval)
- Communication: `std::sync::mpsc` channels with `ZenohCommand` (GUI→Worker) and `ZenohEvent` (Worker→GUI)

### Dual-Session Zenoh Pattern
Two separate Zenoh sessions run concurrently to avoid CONNECTION_TO_SELF:
- **Publishing session**: User-initiated subscribe/publish/query operations
- **Monitor session**: Background `**` wildcard subscription for real-time traffic observation. Uses port+1000 with scouting disabled. Connects lazily after publishing session is ready.

### Key Enums (command/event protocol)
- `ZenohCommand`: Connect, Disconnect, Subscribe, Unsubscribe, Publish, Query, EnableQueryable, DisableQueryable, Ping
- `ZenohEvent`: PublishingConnected, MonitorConnected, Disconnected, ConnectionError, MessageReceived, MessageBatch, SubscriptionCreated, SubscriptionRemoved, QueryNoResponses, DiscoveryUpdate, Pong

### Key Data Structures
- `SendItApp` (in `app.rs`) — Main app state
- `ZenohNode` — Hierarchical BTreeMap-based topic tree node (sorted keys, message counts, payload previews capped at 10KB)
- `ZenohMessage` — Message with key, payload string, raw `payload_bytes: Option<Vec<u8>>`, encoding, timestamp, type, source
- `RateLimiter` — Token bucket rate limiter (10-10k msg/s)

### Shared State (Arc<RwLock<>>)
- `browse_tree: Arc<RwLock<ZenohNode>>` — Hierarchical topic tree updated by both sessions
- `payload_store: Arc<RwLock<HashMap<String, (Vec<u8>, DateTime)>>>` — Full raw payload bytes for export (up to 4GB per entry)
- `local_kvstore: Arc<RwLock<HashMap<String, (String, String)>>>` — Text previews for queryable responses (≤10MB)

## File Transfer Pipeline

This is the core value of the application. Five stages:

### 1. Import (UI side — `ui/drop_zone.rs`)
- Drag-and-drop via `egui::RawInput::dropped_files` (primary) or native file dialog via `rfd::FileDialog`
- Single file only — multi-drop rejected
- Topic key derived from file path: parent directory + filename (e.g. `Documents/photo.jpg`)
- Auto-sends immediately on drop
- Imported files marked with `from_import: true` in `ZenohCommand::Publish`

### 2. Publish Dispatch (worker side — `zenoh_worker.rs`)
Four tiers based on payload size:

| Size | Mechanism | Congestion Control | Echo to UI | Storage |
|------|-----------|-------------------|------------|---------|
| >4GB (>u32::MAX) | 64MB chunks to `{key}/__chunk/{total_size}/{total_chunks}/{chunk_index}` | Block | No | Chunks stored individually |
| 100MB–4GB | Single payload | Block | No (too large) | Via subscription |
| Any (from_import=true) | Single payload | Block | No (ephemeral) | Memory freed after publish |
| <100MB text | Single payload | Block | Yes, as LocalEcho with raw bytes | Echoed + stored |

- Chunk key format: `{key}/__chunk/{total_size}/{total_chunks}/{chunk_index}`
- `CHUNK_SIZE = 64 * 1024 * 1024` (64MB)
- `MAX_SINGLE_PAYLOAD = 0xFFFF_FFFF` (u32::MAX, ~4GB) — Zenoh codec limit

### 3. Receive & Store (worker → GUI — `events.rs`)
- Messages arrive via `ZenohEvent::MessageBatch` (batched) or `MessageReceived` (single)
- Raw bytes stored in `payload_store` HashMap keyed by full topic path
- Preview (≤10KB) stored in `browse_tree` node's `last_payload` field
- Chunk messages stored individually by their full chunk key
- Deduplication via DefaultHasher: hashes key + payload length + first 4KB + last 4KB
- 60-second dedup window

### 4. Chunk Tracking (UI side — `transfer.rs`)
- Scans `payload_store` keys matching `{topic}/__chunk/{total_size}/{total_chunks}/{chunk_index}`
- Parses metadata from the key path string
- Displays progress: "N/M chunks received, X.XX GB total"

### 5. Export / Reassembly (UI side — `transfer.rs` + `ui/topic_tree.rs`)
- **Direct lookup**: First checks `payload_store` for exact topic key match
- **Chunk reassembly fallback**: Collects all `{topic}/__chunk/` keys, sorts by chunk_index, concatenates, verifies completeness
- **File save**: Native dialog via `rfd::FileDialog` with suggested filename
- Available in the slide-out right detail panel when a topic is selected

### Known Issue: payload_store LRU Eviction
The `payload_store` has a 500-entry LRU limit. A 100GB file produces ~1,600 chunks. Older chunks get evicted before all arrive, making reassembly impossible for files >~32GB. Options: increase cap for chunk keys, separate chunk-aware store, or streaming reassembly to disk.

## Zenoh Connection Configuration
- Max message size: 100GB (overrides default 1GB)
- Transport batch size: 1472 (UDP) or 65535 (TCP)
- RX buffer: 16MB for large fragmented messages
- Queue size: 16 (default 2), batching DISABLED
- Block congestion control timeout: 300 seconds (5 minutes) for large transfers
- Peer mode: multicast discovery (224.0.0.224:7446), gossip routing, listens on configurable port
- Client mode: disables multicast, connects to specified router endpoint

## UI Framework Notes
- **egui 0.29** immediate-mode GUI with **eframe 0.29** (Glow/OpenGL renderer)
- Frame rate: ~15fps via `request_repaint_after(66ms)`
- Dark/light mode with iOS-inspired color palette (`SendItColors` struct in `colors.rs`)
- Window: 1400x900 default, 1000x600 minimum

## Key Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| zenoh | 1.0 (unstable) | Core Zenoh networking protocol |
| egui/eframe | 0.29 | Immediate-mode GUI (glow renderer) |
| tokio | 1.0 (full) | Async runtime for worker thread |
| rfd | 0.14 | Native file dialogs (import/export) |
| seahash | 4.1 | Fast payload hashing for dedup |
| chrono | 0.4 | Timestamps with serde |
| anyhow | 1.0 | Error handling |
| serde/serde_json | 1.0 | JSON config parsing |

## Platform Notes
- Windows: `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` hides console
- Windows: panic hook writes crash.log next to executable
- Docker: Alpine-based, exposes port 7447
