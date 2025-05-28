# RequestRepo Benchmarks

This directory contains benchmarks for comparing the performance of the Python FastAPI and Rust implementations of RequestRepo.

## Requirements

- [wrk](https://github.com/wg/wrk) - HTTP benchmarking tool
- Python 3.6+ with matplotlib and numpy
- Both Python and Rust implementations of RequestRepo

## Installation

Install wrk:

```bash
sudo apt-get update
sudo apt-get install -y wrk
```

Install Python dependencies:

```bash
pip install matplotlib numpy
```

## Running Benchmarks

To run the benchmarks:

```bash
cd benchmarks
chmod +x run_benchmark.sh
./run_benchmark.sh
```

The script will:

1. Start the Python FastAPI implementation
2. Run benchmarks against key endpoints with various concurrency levels
3. Stop the Python implementation
4. Start the Rust implementation
5. Run the same benchmarks
6. Generate comparison results and plots

## Benchmark Parameters

- Duration: 30 seconds per test
- Connections: 10, 50, 100, 200
- Threads: 4
- Endpoints: `/api/token`, `/api/dns`

## Results

After running the benchmarks, results will be available in:

- `results/` directory - Raw benchmark output files
- `results/plots/` - Comparison charts
- `summary.md` - Markdown summary with analysis

## Interpreting Results

The benchmark measures:

- **Requests per second** - Higher is better
- **Latency** - Lower is better
- **Transfer rate** - Higher is better

The summary includes percentage improvements and visual comparisons between the implementations.
