# RequestRepo Benchmarks

This directory contains benchmarking tools to compare the performance of the Python (FastAPI) and Rust implementations of the RequestRepo backend.

## Requirements

- Python 3.7+
- `requests` library
- `rich` library

Install dependencies:

```bash
pip install requests rich
```

## Running Benchmarks

### Start the Servers

Before running benchmarks, make sure both servers are running:

1. Start the Python server:
```bash
cd ~/repos/requestrepo
make start-backend
```

2. Start the Rust server (on a different port):
```bash
cd ~/repos/requestrepo/src
RUST_HTTP_PORT=21338 cargo run --release
```

### Run the Benchmark

```bash
cd ~/repos/requestrepo/benchmarks
python benchmark.py
```

### Options

- `--python-url`: URL of the Python implementation (default: http://localhost:21337)
- `--rust-url`: URL of the Rust implementation (default: http://localhost:21338)
- `--concurrency`: Concurrency levels to test (default: 1 5 10 50 100)
- `--duration`: Duration of each benchmark in seconds (default: 10)
- `--output`: Output file for results (default: benchmark_results.json)
- `--config`: Path to benchmark configuration file
- `--python-only`: Only benchmark Python implementation
- `--rust-only`: Only benchmark Rust implementation

Example:

```bash
python benchmark.py --concurrency 1 10 100 --duration 5
```

## Custom Configuration

You can create a custom configuration file to define which endpoints to benchmark:

```json
{
  "python_url": "http://localhost:21337",
  "rust_url": "http://localhost:21338",
  "concurrency": [1, 10, 100],
  "duration": 10,
  "endpoints": [
    {
      "name": "Get Token",
      "method": "POST",
      "path": "/api/get_token",
      "data": {},
      "headers": {"Content-Type": "application/json"}
    },
    {
      "name": "Get DNS",
      "method": "GET",
      "path": "/api/get_dns?token=YOUR_TOKEN",
      "data": null,
      "headers": {}
    }
  ]
}
```

Then run with:

```bash
python benchmark.py --config my_config.json
```

## Interpreting Results

The benchmark script will output a table comparing the performance of both implementations, including:

- Requests per second (RPS)
- Average latency
- Median latency
- Maximum latency
- Error count

It will also calculate the speedup factor of Rust compared to Python for each endpoint and concurrency level.

Results are saved to a JSON file for further analysis.
