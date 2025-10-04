# Using Quinn QUIC Stack for Deep Space 

# Installation
- [install Rust](https://rustup.rs/)
- `git clone https://github.com/deepspaceip/dipt-http`
- `cd dipt-http/quinn`

# Key Considerations
For deep space simulation, with long delays and intermittence, the QUIC stacks default configuration is not suitable. Therefore, the connection must be configured accordingly. 

As discussed in [draft-many-tiptop-quic-profile](), calculate the expected maximum RTT value of the connection, based on the delays and intermittence, and set the initial-rtt and idle-timeout to that value. Also set the congestion controller to a noop. 

The cloned repo contains a modified version of the Quinn example http client and server with additional options and a noop congestion controller.

The relevant options to set the proper transport configuration are:

- `initial-rtt` (unit: ms)
- `idle-timeout` (unit: ms)
- `cc noop`


# Quinn as HTTP Client over QUIC
The following example is for a max rtt is 3600 seconds (1h) and a Quinn server with a self-signed certificate

```bash
cargo run --release --bin client -- \
--no-verify --maxrtt 3600000 --cc noop URL
```
- `no-verify` is to not verify the server certificate, useful when the server is using a self-signed certificate
- `maxrtt` sets the initial-rtt and idle-timeout to the proper value
- replace `URL` by the URL of the quic server (ex: `https://127.0.0.1:4433`)
 
# Quinn as HTTP Server over QUIC
The following example is for a max rtt of 3600 seconds (1h).

```bash
cargo run --release --bin server -- \
--maxrtt 3600000 --listen 0.0.0.0:4433 DIR
```
- replace `DIR` by the directory where html pages are located
- use `--listen` to specify address and port to bind the server to.
- use `--alpn hq-interop` if the client is Quiche

# Get help

## Arguments
- `cargo run --release --bin client -- --help`
- `cargo run --release --bin server -- --help`

## On execution
- add `RUST_LOG=info` in front of the cargo command to get more details on the console