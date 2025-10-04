# Using Cloudflare Quiche QUIC Stack for Deep Space 

# Installation
- [install Rust](https://rustup.rs/)

Contrary to the Quinn QUIC stack where we can add a congestion controller outside of the source repo, it is not possible to do so with Quiche. Therefore, a fork was done and the congestion controller was added.

- `git clone https://github.com/deepspaceip/quiche`
- `cd quiche`


# Key Considerations
For deep space simulation, with long delays and intermittence, the QUIC stacks default configuration is not suitable. Therefore, the connection must be configured accordingly. 

As discussed in [draft-many-tiptop-quic-profile](), calculate the expected maximum RTT value of the connection, based on the delays and intermittence, and set the initial-rtt and idle-timeout to that value. Also set the congestion controller to a noop. 

The relevant Quiche options to set the proper transport configuration are:

- `initial-rtt` (unit: ms)
- `idle-timeout` (unit: ms)
- `cc-algorithm congestion_window_unchecked`

We found that for interoperability with the Quinn stack, the following parameters were required for the Quiche client:
`--wire-version 1 --http-version 'HTTP/0.9' --dgram-proto none`

# Quiche as HTTP Client over QUIC
The following example is for a max rtt is 3600 seconds (1h) and a Quinn server with a self-signed certificate

```bash
cargo run --release --bin quiche-client -- \
--no-verify --idle-timeout 3600000 --initial-rtt 3600000 \
--wire-version 1 --http-version HTTP/0.9 \
--dgram-proto none --cc-algorithm congestion_window_unchecked URL
```
- `no-verify` is to not verify the server certificate, useful when the server is using a self-signed certificate
- replace `URL` by the URL of the quic server
 
# Quiche as HTTP Server over QUIC
The following example is for a max rtt of 3600 seconds (1h).

```bash
cargo run --release  --bin quiche-server -- \
--idle-timeout 3600000 --initial-rtt 3600000 \
--cc-algorithm congestion_window_unchecked 
--listen 0.0.0.0:4433 --root DIR
```
- replace `DIR` by the directory where html pages are located
- use `--listen` to specify address and port to bind the server to.

# Get help on arguments

- `cargo run --release --bin quiche-client -- --help`
- `cargo run --release --bin quiche-server -- --help`