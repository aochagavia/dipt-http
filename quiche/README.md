# Using Cloudflare Quiche QUIC Stack for Deep Space 

# Installation
- [install Rust](https://rustup.rs/)
- git clone https://github.com/cloudflare/quiche
- cd quiche

# Key Considerations
For deep space simulation, with long delays and intermittence, the QUIC stacks default configuration is not suitable. Therefore, the connection must be configured accordingly. 

As discussed in [draft-many-tiptop-quic-profile](), calculate the expected maximum RTT value of the connection, based on the delays and intermittence, and set the initial-rtt and idle-timeout to that value. Also set the congestion controller to a noop. 

The relevant Quiche options to set the proper transport configuration are:

- initial-rtt (unit: ms)
- idle-timeout (unit: ms)
- cc-algorithm congestion\_window\_unchecked

We found that for interoperability with the Quinn stack, the following parameters were required for the Quiche client:
"--wire-version 1 --http-version 'HTTP/0.9' --dgram-proto none"

# Quiche as HTTP Client over QUIC
The following example is for a max rtt is 3600 seconds (1h) and a Quinn server with a self-signed certificate

- cargo run --release --bin quiche-client -- --no-verify --idle-timeout 3600000 --initial-rtt 3600000 --wire-version 1 --http-version HTTP/0.9 --dgram-proto none --cc-algorithm congestion\_window\_unchecked URL
  - no-verify is to not verify the server certificate, useful when the server is using a self-signed certificate
  - replace URL by the URL of the quic server
 
# Quiche as HTTP Server over QUIC
The following example is for a max rtt of 3600 seconds (1h).

- cargo run --release  --bin quiche-server -- --idle-timeout 3600000 --initial-rtt 3600000 --root DIR
  - replace DIR by the directory where html pages are located
  - use --listen to specify address and port to bind the server to. For example: --listen 0.0.0.0:4433

# Get help on arguments

- cargo run --release --bin quiche-client -- --help
- cargo run --release --bin quiche-server -- --help