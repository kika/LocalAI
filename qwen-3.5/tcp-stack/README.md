# TCP Stack - Userspace TCP Implementation in Rust

A complete userspace TCP protocol implementation using Linux raw IP sockets. This project provides an educational implementation of the TCP protocol stack from scratch.

## Features

### Implemented
- **IPv4 Header** - Full IPv4 header parsing and serialization
- **TCP Header** - Complete TCP header with options support
  - Maximum Segment Size (MSS)
  - Window Scaling
  - Selective Acknowledgments (SACK)
  - Timestamps
- **TCP State Machine** - Full RFC 793 state machine
  - CLOSED, LISTEN, SYN-SENT, SYN-RECEIVED
  - ESTABLISHED, FIN-WAIT-1, FIN-WAIT-2
  - CLOSE-WAIT, CLOSING, LAST-ACK, TIME-WAIT
- **Reliability**
  - Sequence number tracking
  - Retransmission with dynamic RTO (Jacobson's algorithm)
  - Out-of-order packet reassembly
  - Fast retransmit (3 duplicate ACKs)
- **Flow Control** - Sliding window mechanism
- **Congestion Control** - NewReno algorithm
  - Slow start
  - Congestion avoidance
  - Fast recovery
- **Raw Socket Interface** - Direct IP packet sending/receiving

## Project Structure

```
tcp-stack/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point
│   ├── lib.rs               # Library exports
│   ├── packet/
│   │   ├── mod.rs
│   │   ├── ip.rs            # IPv4 header
│   │   └── tcp.rs           # TCP header + options
│   ├── socket/
│   │   ├── mod.rs
│   │   └── raw.rs           # Raw socket wrapper
│   ├── connection/
│   │   ├── mod.rs           # Connection struct
│   │   ├── states.rs        # TCP states
│   │   ├── control.rs       # Protocol Control Block
│   │   └── timer.rs         # Timers
│   ├── reliability/
│   │   ├── mod.rs
│   │   ├── retransmit.rs    # Retransmission logic
│   │   └── reorder.rs       # Out-of-order handling
│   ├── flow_control/
│   │   ├── mod.rs
│   │   └── window.rs        # Sliding window
│   ├── congestion/
│   │   ├── mod.rs
│   │   └── newreno.rs       # NewReno congestion control
│   ├── demux/
│   │   └── mod.rs           # Packet demultiplexing
│   └── utils/
│       ├── mod.rs
│       ├── checksum.rs      # TCP/IP checksum
│       └── seq.rs           # Sequence number arithmetic
├── examples/
│   ├── echo_server.rs       # Echo server demo
│   └── http_client.rs       # HTTP client demo
└── tests/
```

## Building

```bash
cd tcp-stack
cargo build --release
```

## Running

### Basic Test
```bash
cargo run
```

### Examples
```bash
# HTTP Client example
RUST_LOG=info cargo run --example http_client

# Echo Server (requires root for raw sockets)
sudo -E $(which cargo) run --example echo_server
```

### Tests
```bash
cargo test
```

## Usage

### Creating a Raw Socket
```rust
use tcp_stack::RawSocket;

let socket = RawSocket::new()?;
```

### Creating a TCP Connection
```rust
use tcp_stack::{RawSocket, TcpConnection};
use std::net::{Ipv4Addr, SocketAddrV4};

let socket = RawSocket::new()?;
let local = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8080);
let remote = SocketAddrV4::new(Ipv4Addr::new(93, 184, 216, 34), 80);

let mut conn = TcpConnection::new(socket, local, remote);
```

### Sending Data
```rust
// Build TCP packet
let header = TcpHeader::new(local.port(), remote.port());
let ip_header = Ipv4Header::new(
    local.ip(),
    remote.ip(),
    header.header_len() + data.len()
);

let packet = [ip_header.serialize(), header.serialize(), data].concat();
socket.send_to(&packet, remote.ip())?;
```

## Architecture

### Packet Flow
1. **Application** writes data to connection
2. **TCP Layer** segments data, adds TCP header with options
3. **IP Layer** adds IPv4 header
4. **Raw Socket** sends packet to network

### Receive Flow
1. **Raw Socket** receives IP packet
2. **IP Layer** parses and validates header
3. **Demultiplexer** routes to correct connection
4. **TCP Layer** processes segment, handles ACKs/retransmits
5. **Application** reads received data

### State Machine
```
    +--------+
    | CLOSED |
    +--------+
       |  |
       |  +---> [Passive Open] ---> LISTEN
       |                              |
       |                              | [Recv SYN]
       |                              v
       |                         SYN-RECEIVED
       |                              |
       |                              | [Recv ACK]
       |                              v
       +------------------------> ESTABLISHED
       |   [Active Open]              |  |
       |       |                      |  | [Close]
       |       v                      |  v
       +---> SYN-SENT            FIN-WAIT-1
               |                      |
               | [Recv SYN+ACK]       | [Recv ACK]
               v                      v
           ESTABLISHED           FIN-WAIT-2
```

## Technical Details

### Checksum Calculation
TCP uses a 16-bit one's complement checksum over:
- Pseudo-header (src IP, dst IP, protocol, TCP length)
- TCP header
- TCP data

### Sequence Numbers
32-bit sequence numbers with wraparound arithmetic. Comparison uses RFC 793 semantics:
```rust
fn before(self, other: SeqNumber) -> bool {
    self.diff(other) > 0x8000_0000
}
```

### RTT Estimation
Jacobson's algorithm with Karn's modification:
```
SRTT = (1 - α) × SRTT + α × RTT
RTTVAR = (1 - β) × RTTVAR + β × |SRTT - RTT|
RTO = SRTT + 4 × RTTVAR
```
Where α = 0.125, β = 0.25

### Congestion Control (NewReno)
- **Slow Start**: cwnd doubles every RTT until ssthresh
- **Congestion Avoidance**: cwnd increases by 1/cwnd per ACK
- **Fast Retransmit**: Retransmit on 3 duplicate ACKs
- **Fast Recovery**: Halve cwnd, continue in congestion avoidance

## Limitations

1. **IPv4 Only** - No IPv6 support yet
2. **Linux Only** - Uses Linux-specific raw socket APIs
3. **No IP Fragmentation** - Assumes path MTU is known
4. **Single-threaded** - Event loop processes one connection at a time
5. **No ECN** - Explicit Congestion Notification not implemented

## Requirements

- Linux kernel 4.0+
- Root privileges (for raw sockets)
- Rust 1.75+

## License

MIT License

## Contributing

This is an educational implementation. Contributions welcome for:
- Bug fixes
- Performance improvements
- Additional TCP options
- IPv6 support
- Better congestion control algorithms (CUBIC, BBR)
