# requestrepo

[![PyPI](https://img.shields.io/pypi/v/requestrepo)](https://pypi.org/project/requestrepo/)
[![Python](https://img.shields.io/pypi/pyversions/requestrepo)](https://pypi.org/project/requestrepo/)
[![License](https://img.shields.io/pypi/l/requestrepo)](https://github.com/adrgs/requestrepo/blob/main/LICENSE)

Python client library for [requestrepo](https://requestrepo.com) - an HTTP/DNS/SMTP/TCP request analysis tool for security researchers and developers.

## Installation

```bash
pip install requestrepo
```

## Quick Start

```python
from requestrepo import Requestrepo

# Create a new session
repo = Requestrepo()
print(f"Send requests to: {repo.subdomain}.{repo.domain}")

# Start listening for requests
repo.start_requests()

# Wait for and process an HTTP request
request = repo.get_http_request()
print(f"Received {request.method} {request.path} from {request.ip}")

# Clean up
repo.stop_requests()
```

## Features

- Capture and inspect HTTP requests with full headers, body, and metadata
- Monitor DNS queries to custom subdomains
- Capture SMTP emails sent to your domain
- Capture raw TCP connections
- Define custom HTTP responses with headers and status codes
- Set custom DNS records (A, AAAA, CNAME, TXT)
- Share individual requests via secure tokens
- Real-time WebSocket streaming for instant notifications

## API Reference

### Session Creation

```python
from requestrepo import Requestrepo

# Basic usage (public instance)
repo = Requestrepo()

# With admin token (for protected instances)
repo = Requestrepo(admin_token="your-admin-token")

# With existing session token
repo = Requestrepo(token="existing-jwt-token")

# Custom instance
repo = Requestrepo(
    host="your-instance.com",
    port=443,
    protocol="https",
    verify=True  # SSL verification
)

# Access session info
print(repo.subdomain)  # e.g., "abc123"
print(repo.domain)     # e.g., "requestrepo.com"
print(repo.token)      # JWT token for this session
```

### DNS Operations

```python
# Add DNS records
repo.add_dns("test", "A", "1.2.3.4")
repo.add_dns("mail", "AAAA", "2001:db8::1")
repo.add_dns("www", "CNAME", "example.com")
repo.add_dns("_dmarc", "TXT", "v=DMARC1; p=none")

# List all DNS records
records = repo.dns()
for record in records:
    print(f"{record.domain} {record.type} {record.value}")

# Remove DNS records
repo.remove_dns("test")           # Remove all records for "test"
repo.remove_dns("test", "A")      # Remove only A records for "test"

# Replace all DNS records
from requestrepo import DnsRecord
repo.update_dns([
    DnsRecord(type="A", domain="@", value="1.2.3.4"),
    DnsRecord(type="TXT", domain="@", value="hello world"),
])
```

### Custom HTTP Responses

Configure what responses are returned when HTTP requests hit your subdomain:

```python
# Set a simple HTML response
repo.set_file("index.html", "<h1>Hello World</h1>", status_code=200)

# Set a JSON response with custom headers
from requestrepo import Header
repo.set_file(
    "api/data.json",
    '{"status": "ok"}',
    status_code=200,
    headers=[
        Header(header="Content-Type", value="application/json"),
        Header(header="X-Custom-Header", value="custom-value"),
    ]
)

# Set binary content
repo.set_file("image.png", open("local.png", "rb").read())

# List all configured files
files = repo.files()
for path, response in files.items():
    print(f"{path}: {response.status_code}")

# Get a specific file
response = repo.get_file("index.html")
```

### Request Management

```python
# List captured requests
requests = repo.list_requests(limit=100, offset=0)
for req in requests:
    print(f"{req.type}: {req.ip} at {req.date}")

# Delete a specific request
repo.delete_request(requests[0].id)

# Delete all requests
repo.delete_all_requests()
```

### Request Sharing

Share individual requests with others without giving access to your full session:

```python
# Get a share token for a request
requests = repo.list_requests(limit=1)
share_token = repo.share_request(requests[0].id)
print(f"Share URL: https://requestrepo.com/r/{share_token}")

# Anyone can view the shared request (no auth required)
shared_request = repo.get_shared_request(share_token)
```

### Real-time WebSocket Streaming

#### Blocking Mode

Process requests as they arrive in a blocking loop:

```python
class MyRepo(Requestrepo):
    def on_request(self, request):
        print(f"Got {request.type} request from {request.ip}")
        if request.type == "http":
            print(f"  {request.method} {request.path}")

    def on_deleted(self, request_id):
        print(f"Request {request_id} was deleted")

    def on_cleared(self):
        print("All requests were cleared")

repo = MyRepo()
repo.await_requests()  # Blocks forever
```

#### Background Mode

Process requests in a background thread while doing other work:

```python
repo = Requestrepo()
repo.start_requests()  # Start listening in background

# Do other work...
# Then wait for specific requests:
http_req = repo.get_http_request()  # Blocks until HTTP request arrives
dns_req = repo.get_dns_request()    # Blocks until DNS request arrives

# Or use custom filters:
post_req = repo.get_request(lambda r: r.type == "http" and r.method == "POST")

# Stop listening when done
repo.stop_requests()
```

#### Built-in Filters

```python
# Filter by request type
http_req = repo.get_request(Requestrepo.HTTP_FILTER)
dns_req = repo.get_request(Requestrepo.DNS_FILTER)
smtp_req = repo.get_request(Requestrepo.SMTP_FILTER)
tcp_req = repo.get_request(Requestrepo.TCP_FILTER)

# Or use convenience methods
http_req = repo.get_http_request()
dns_req = repo.get_dns_request()
smtp_req = repo.get_smtp_request()
tcp_req = repo.get_tcp_request()
```

## Request Types

### HttpRequest

```python
request.id          # Unique identifier
request.type        # "http"
request.ip          # Source IP address
request.country     # Two-letter country code (e.g., "US")
request.date        # Unix timestamp
request.method      # HTTP method (GET, POST, etc.)
request.path        # Request path with query string
request.http_version # e.g., "HTTP/1.1"
request.headers     # Dict of headers
request.body        # Request body as bytes
request.raw         # Raw request data as bytes
```

### DnsRequest

```python
request.id          # Unique identifier
request.type        # "dns"
request.ip          # Source IP address
request.country     # Two-letter country code
request.port        # Source port
request.date        # Unix timestamp
request.query_type  # DNS query type (A, AAAA, TXT, etc.)
request.domain      # Queried domain name
request.reply       # DNS reply sent back
request.raw         # Raw DNS query as bytes
```

### SmtpRequest

```python
request.id          # Unique identifier
request.type        # "smtp"
request.ip          # Source IP address
request.country     # Two-letter country code
request.date        # Unix timestamp
request.command     # SMTP command
request.data        # Email body
request.subject     # Email subject
request.from_addr   # Sender address
request.to          # Recipient address
request.cc          # CC recipients
request.bcc         # BCC recipients
request.raw         # Raw SMTP data as bytes
```

### TcpRequest

```python
request.id          # Unique identifier
request.type        # "tcp"
request.ip          # Source IP address
request.country     # Two-letter country code
request.port        # TCP port number
request.date        # Unix timestamp
request.raw         # Raw TCP data as bytes
```

## Complete Example

```python
from requestrepo import Requestrepo, Header

class SecurityTester(Requestrepo):
    def on_request(self, request):
        if request.type == "http":
            print(f"[HTTP] {request.method} {request.path}")
            if "Authorization" in request.headers:
                print(f"  Auth: {request.headers['Authorization']}")
        elif request.type == "dns":
            print(f"[DNS] {request.query_type} {request.domain}")
        elif request.type == "smtp":
            print(f"[SMTP] From: {request.from_addr} To: {request.to}")
        elif request.type == "tcp":
            print(f"[TCP] {len(request.raw)} bytes on port {request.port}")

# Create session
repo = SecurityTester()
print(f"Your endpoint: {repo.subdomain}.{repo.domain}")

# Configure DNS for testing
repo.add_dns("@", "A", "127.0.0.1")
repo.add_dns("canary", "TXT", "if-you-see-this-call-home")

# Configure HTTP response
repo.set_file("callback", '{"status":"received"}',
    status_code=200,
    headers=[Header(header="Content-Type", value="application/json")]
)

# Start listening
print("Waiting for requests...")
repo.await_requests()
```

## License

MIT License - see [LICENSE](LICENSE) for details.
