# RequestRepo Rust Backend

This is a complete rewrite of the RequestRepo backend and DNS server in Rust, replacing Redis with an in-memory compressed cache.

## Features

- HTTP server with all API endpoints from the original Python implementation
- DNS server with support for A, AAAA, CNAME, and TXT records
- WebSocket endpoint for real-time updates
- In-memory compressed cache to replace Redis
- IP geolocation for request tagging
- SMTP logging on port 25 (new feature)
- Custom TCP port per session (new feature)

## Architecture

The application is structured into several modules:

- `cache`: In-memory compressed cache implementation
- `dns`: DNS server implementation
- `http`: HTTP server with API endpoints and WebSocket support
- `ip2country`: IP geolocation functionality
- `models`: Data models for the application
- `smtp`: SMTP server for email logging
- `tcp`: TCP server for custom ports per session
- `utils`: Utility functions and configuration

## Building and Running

```bash
# Build the application
cargo build --release

# Run the application
cargo run --release
```

## Environment Variables

The application uses the following environment variables:

- `REDIS_HOST`: Redis host (for compatibility, not used)
- `SERVER_IP`: Server IP address
- `DOMAIN`: Server domain
- `INCLUDE_SERVER_DOMAIN`: Whether to include server domain in responses
- `SUBDOMAIN_LENGTH`: Length of generated subdomains
- `SUBDOMAIN_ALPHABET`: Alphabet for generating subdomains
- `JWT_SECRET`: JWT secret
- `MAX_FILE_SIZE`: Maximum file size
- `MAX_REQUEST_SIZE`: Maximum request size
- `TXT`: Default TXT record value
- `REDIS_TTL_DAYS`: Cache TTL in days
- `HTTP_PORT`: HTTP server port
- `DNS_PORT`: DNS server port
- `SMTP_PORT`: SMTP server port
- `TCP_PORT_RANGE_START`: TCP port range start
- `TCP_PORT_RANGE_END`: TCP port range end

## Testing

```bash
# Run tests
cargo test

# Run ignored tests (requires running servers)
cargo test -- --ignored
```
