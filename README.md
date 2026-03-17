# SendIT

```
    *              .                    *              .
                        *         +           .
         .                    *                         *
               +       .              *     .
    .       *        .                             +
                          *      .
    +     *    .                +         *


                            ___
                           /   \
                          | . . |
                          |     |
                          | >>> |
                          |     |
                          |_____|
                         /|     |\
                        / |     | \
                       /  |_____|  \
                           |   |
                          /|   |\
                         / |   | \
                        '  /   \  '
                       `  / ~ ~ \  `
                      ~  /  ~ ~  \  ~
                     '  / ~ ~ ~ ~ \  '
                    `  /~ ~ ~ ~ ~ ~\  `
                   ~  /~ ~ ~ ~ ~ ~ ~\  ~
                  '  ~ ~ ~ ~ ~ ~ ~ ~ ~  '


                            .~.
          /\               /   \               /\
         /  \    /\       /     \      /\     /  \

                       s e n d   i t .
```

A simple, high-performance drag-and-drop file transfer tool for Linux, Windows and Mac.

## In a Nutshell

Open the app, automatically discover destinations, drop a file onto the app window and the file is instantly published. Other SendIT instances on the same Ethernet network see it appear in the topic tree and can export/save it. No server, no configuration, just simple file transfer over standards-based peer-to-peer plumbing.

### Stuff It Does

- **Automated-Connection**: Auto-discovers and auto-detects other SendIt instances — no setup required
- **Drag-and-drop sending**: Drop a file, it sends automatically
- **Large file support**: Send single files up to 50GB in size
- **High performance**: Leverages powerful new technologies for fast and efficient operations within the application and on top of the Ethernet layer ([Rust] [Zenoh])
- **Dark/light mode**: Toggle between themes

## Quick Start

### Prerequisites
- Rust 1.70+

### Build & Run

```bash
git clone <repository-url>
cd send_it
cargo run --release
```

The app auto-connects in peer mode on port 7447 with multicast discovery. Drop a file to send it.

### Docker

```bash
docker build -t send-it .
docker run -p 7447:7447 send-it
```

## Usage

### Sending Files
1. Launch SendIT
2. Wait for connection (green dot in toolbar)
3. Drag a file onto the central drop zone
4. File is published automatically — topic key is derived from the file's parent directory and filename (e.g. `Documents/photo.jpg`)

### Receiving Files
1. Files from other peers appear in the topic tree (left panel)
2. Click a topic to see details in the slide-out right panel
3. Click "Export Payload" to save the file locally
4. For chunked files, progress shows completion status before export

### Settings
Click the gear icon (top of left panel) to access:
- **Connection**: Transport, address, port, peer/client mode, connect/disconnect
- **Subscriptions**: Subscribe to key expressions, manage active subscriptions
- **Query**: Send queries to the network, configure timeouts
- **Queryable**: Enable a service that responds to queries with locally stored data
- **Memory & Performance**: Memory limits, message limits, rate limiting, deduplication

### Connection Modes
- **Peer mode** (default): Multicast discovery, mesh networking. Leave address blank for auto-discovery, or specify `tcp/ip:port` for specific peers.
- **Client mode**: Connect to a Zenoh router at a specific endpoint.

## Architecture

- **Rust + egui/eframe**: Native desktop GUI with OpenGL rendering
- **Dual-thread model**: GUI thread + Tokio async worker thread + message buffer thread
- **Dual Zenoh sessions**: Publishing session (user operations) + monitor session (background `**` subscription)
- **File transfer pipeline**: Import → publish dispatch (4 size tiers) → receive & store → chunk tracking → export/reassembly

See [CLAUDE.md](CLAUDE.md) for detailed architecture documentation.

## License

Apache-2.0
