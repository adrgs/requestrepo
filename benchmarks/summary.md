# RequestRepo Benchmark Results

Comparison between Python FastAPI and Rust implementations

## Performance Summary

- **Average Throughput Improvement**: 6141.56%
- **Average Latency Improvement**: 98.05%

## Performance Graphs

### dns Endpoint

#### Requests per Second

![dns RPS](results/plots/dns_rps.png)

#### Latency

![dns Latency](results/plots/dns_latency.png)

#### Transfer Rate

![dns Transfer](results/plots/dns_transfer.png)

### token Endpoint

#### Requests per Second

![token RPS](results/plots/token_rps.png)

#### Latency

![token Latency](results/plots/token_latency.png)

#### Transfer Rate

![token Transfer](results/plots/token_transfer.png)

## Detailed Results

### dns Endpoint

| Connections | Implementation | Requests/sec | Latency (ms) | Transfer (MB/s) |
|-------------|---------------|--------------|--------------|----------------|
| 10 | Python FastAPI | 1797.62 | 4.44 | 0.29 |
| 10 | Rust | 53682.14 | 0.15 | 10.24 |
| 50 | Python FastAPI | 1912.33 | 25.64 | 0.31 |
| 50 | Rust | 94062.52 | 0.55 | 17.94 |
| 100 | Python FastAPI | 1898.32 | 62.30 | 0.30 |
| 100 | Rust | 127685.31 | 0.85 | 24.35 |
| 200 | Python FastAPI | 1868.24 | 100.33 | 0.30 |
| 200 | Rust | 191211.34 | 1.07 | 36.47 |

### token Endpoint

| Connections | Implementation | Requests/sec | Latency (ms) | Transfer (MB/s) |
|-------------|---------------|--------------|--------------|----------------|
| 10 | Python FastAPI | 1805.51 | 4.42 | 0.29 |
| 10 | Rust | 55248.72 | 0.14 | 15.81 |
| 50 | Python FastAPI | 1889.16 | 26.00 | 0.30 |
| 50 | Rust | 95960.10 | 0.53 | 27.45 |
| 100 | Python FastAPI | 1851.55 | 62.69 | 0.30 |
| 100 | Rust | 126287.88 | 0.86 | 36.13 |
| 200 | Python FastAPI | 1835.33 | 101.88 | 0.29 |
| 200 | Rust | 185482.90 | 1.09 | 53.07 |

## Conclusion

The Rust implementation shows significant performance improvements over the Python FastAPI implementation in both throughput and latency. This demonstrates the benefits of using a compiled language like Rust for high-performance network services.

### Key Observations

- The Rust implementation uses an in-memory compressed cache instead of Redis, which may contribute to performance differences
- Performance differences become more pronounced under higher concurrency levels
- The Rust implementation includes additional features like SMTP logging and custom TCP port allocation
