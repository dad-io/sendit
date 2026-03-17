# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**SendIT** (currently named Zenoh Explorer) — a native desktop GUI application for file transfer over Zenoh networks. Built in Rust with egui/eframe. Being refactored from a monolithic single-file network debugger into a focused drag-and-drop file transfer tool.

## Build & Run

```bash
cargo build                                    # Debug build
cargo build --release                          # Release (LTO, stripped, panic=abort)
cargo run                                      # Run debug
cargo run --release                            # Run release
RUST_LOG=zenoh_explorer=debug cargo run        # Run with debug logging
```

No test suite. No CI/CD. Docker build available via `Dockerfile` (rust:1.93-alpine).

## Module Structure

```
src/
├── main.rs              # Entry point only (~70 lines)
├── app.rs               # ZenohExplorer struct, construction, theme helpers, eframe::App impl
├── events.rs            # Event processing, dedup, browse tree updates, message storage
├── colors.rs            # ExplorerColors light/dark palette constants
├── types.rs             # Shared types, enums, constants (ZenohCommand, ZenohEvent, etc.)
├── transfer.rs          # Chunk reassembly, export to file, chunk info scanning
├── zenoh_worker.rs      # Async Tokio worker: connections, publish dispatch, subscribe, query
└── ui/
    ├── mod.rs           # UI module declarations
    ├── topic_tree.rs    # TopicTreeUI trait: tree panel, topic details, tree node rendering
    ├── publish.rs       # PublishUI trait: publish tab + queryable section
    ├── query.rs         # QueryUI trait: query tab + results display
    ├── messages.rs      # MessagesUI trait: message log with filtering
    └── help.rs          # HelpUI trait: help/usage tab
```

### Module Pattern
UI rendering uses **trait-based decomposition**: each `ui/*.rs` file defines a trait (e.g. `TopicTreeUI`) that `ZenohExplorer` implements. The traits must be imported where their methods are called:
- `app.rs` imports `TopicTreeUI` (for `show_tree_panel`/`show_detail_panel` in the eframe::App impl)
- `topic_tree.rs` imports `PublishUI`, `QueryUI`, `HelpUI`, `MessagesUI` (for `show_detail_panel` dispatch)

Event processing methods (`process_events`, `is_duplicate`, `add_message_to_browse_tree`, `add_message_with_limits`, `get_cached_json`) are direct `impl ZenohExplorer` methods in `events.rs`.

All struct fields are `pub(crate)` to allow access from sibling modules.

## Architecture

### Dual-Thread Model
- **GUI thread** (main): egui/eframe rendering + user interaction + state management
- **Worker thread**: Tokio async runtime for all Zenoh network operations
- **Buffer thread**: Batches messages from worker → GUI (50 msgs OR 16ms flush interval)
- Communication: `std::sync::mpsc` channels with `ZenohCommand` (GUI→Worker) and `ZenohEvent` (Worker→GUI)

### Dual-Session Zenoh Pattern
Two separate Zenoh sessions run concurrently to avoid CONNECTION_TO_SELF:
- **Publishing session**: User-initiated subscribe/publish/query operations
- **Monitor session**: Background `**` wildcard subscription for real-time traffic observation. Uses port+1000 with scouting disabled.

### Key Enums (command/event protocol)
- `ZenohCommand`: Connect, Disconnect, Subscribe, Unsubscribe, Publish, Query, EnableQueryable, DisableQueryable, Ping
- `ZenohEvent`: Connected, PublishingConnected, MonitorConnected, Disconnected, ConnectionError, MessageReceived, MessageBatch, SubscriptionCreated, SubscriptionRemoved, QueryNoResponses, DiscoveryUpdate, Pong

### Key Data Structures
- `ZenohExplorer` (in `app.rs`) — Main app state (being renamed to `SendItApp`)
- `ZenohNode` — Hierarchical BTreeMap-based topic tree node (sorted keys, message counts, payload previews capped at 10KB)
- `ZenohMessage` — Message with key, payload string, raw `payload_bytes: Option<Vec<u8>>`, encoding, timestamp, type, source
- `RateLimiter` — Token bucket rate limiter (10-10k msg/s)

### Shared State (Arc<RwLock<>>)
- `browse_tree: Arc<RwLock<ZenohNode>>` — Hierarchical topic tree updated by both sessions
- `payload_store: Arc<RwLock<HashMap<String, (Vec<u8>, DateTime)>>>` — Full raw payload bytes for export (up to 4GB per entry)
- `local_kvstore: Arc<RwLock<HashMap<String, (String, String)>>>` — Text previews for queryable responses (≤10MB)

## File Transfer Pipeline

This is the core value of the application. Five stages that MUST be preserved through refactoring:

### 1. Import (UI side — `ui/publish.rs`)
- Native file dialog via `rfd::FileDialog` OR drag-and-drop (`egui::RawInput::dropped_files`)
- Full bytes read into `publish_payload_bytes: Option<Vec<u8>>`
- Preview generation: 256B collapsed / 4KB expanded
  - Text files: UTF-8 with `safe_truncate_index()` to avoid splitting multi-byte chars
  - Binary files: hex dump (`{:02x}` per byte)
- Memory tracked via `import_memory_bytes` field
- Imported files marked with `from_import: bool` in `ZenohCommand::Publish`

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
- Chunk messages stored individually by their full chunk key (e.g. `topic/__chunk/8589934592/128/0`)
- Deduplication via DefaultHasher: hashes key + payload length + first 4KB + last 4KB (`MAX_HASH_BYTES = 4096`)
- 60-second dedup window

### 4. Chunk Tracking (UI side — `transfer.rs`)
- Scans `payload_store` keys matching `{topic}/__chunk/{total_size}/{total_chunks}/{chunk_index}`
- Parses metadata (total_size, total_chunks, chunk_index) from the key path string
- Displays progress: "N/M chunks received, X.XX GB total"
- Shows completion indicator when all chunks present

### 5. Export / Reassembly (UI side — `transfer.rs` + `ui/topic_tree.rs`)
- **Direct lookup**: First checks `payload_store` for exact topic key match
- **Chunk reassembly fallback**: Collects all `{topic}/__chunk/` keys from payload_store, sorts by chunk_index, concatenates `Vec<u8>` data, verifies all chunks present before writing
- **File save**: Native dialog via `rfd::FileDialog` with suggested filename (topic path with `/` → `_`)
- File type filters: Binary (.bin), Text (.txt), JSON (.json), All (*)

### Known Issue: payload_store LRU Eviction
The `payload_store` has a 500-entry LRU limit. A 100GB file produces ~1,600 chunks (100GB / 64MB). Older chunks get evicted before all arrive, making reassembly impossible for files >~32GB. Must be addressed — options: increase cap for chunk keys, separate chunk-aware store, or streaming reassembly to disk.

## Zenoh Connection Configuration
- Max message size: 100GB (overrides default 1GB)
- Transport batch size: 1472 (UDP) or 65535 (TCP)
- RX buffer: 16MB for large fragmented messages
- Queue size: 16 (default 2), batching DISABLED
- Block congestion control timeout: 300 seconds (5 minutes) for large transfers
- Peer mode: multicast discovery (224.0.0.224:7446), gossip routing, listens on configurable port
- Client mode: disables multicast, connects to specified router endpoint
- Connection timeout: 30s (publishing), 15s (monitor)

## UI Framework Notes
- **egui 0.29** immediate-mode GUI with **eframe 0.29** (Glow/OpenGL renderer)
- Frame rate: ~15fps via `request_repaint_after(66ms)`
- Message rendering capped at last 500 for performance
- JSON pretty-print cache: last 100 entries, skips payloads >50KB
- Payload display: collapsed 1024 chars / expanded full (tree preview ≤ 10KB)
- Dark/light mode with iOS-inspired color palette (`ExplorerColors` struct in `colors.rs`)
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
