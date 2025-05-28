#!/usr/bin/env python3
"""
Benchmark script to compare the performance of the Rust and Python implementations
of the RequestRepo backend.
"""

import argparse
import json
import os
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime
from statistics import mean, median, stdev
from typing import Dict, List, Optional, Tuple

import requests
from rich.console import Console
from rich.table import Table

console = Console()

DEFAULT_CONFIG = {
    "python_url": "http://localhost:21337",
    "rust_url": "http://localhost:21338",
    "concurrency": [1, 5, 10, 50, 100],
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
            "name": "Catch-all",
            "method": "GET",
            "path": "/",
            "data": None,
            "headers": {}
        }
    ]
}

class BenchmarkResult:
    """Class to store benchmark results."""
    
    def __init__(self, name: str, concurrency: int, requests: int, duration: float,
                 latencies: List[float], errors: int):
        self.name = name
        self.concurrency = concurrency
        self.requests = requests
        self.duration = duration
        self.latencies = latencies
        self.errors = errors
        
        self.rps = requests / duration
        self.avg_latency = mean(latencies) if latencies else 0
        self.median_latency = median(latencies) if latencies else 0
        self.min_latency = min(latencies) if latencies else 0
        self.max_latency = max(latencies) if latencies else 0
        self.stdev_latency = stdev(latencies) if len(latencies) > 1 else 0
        
    def to_dict(self) -> Dict:
        """Convert result to dictionary."""
        return {
            "name": self.name,
            "concurrency": self.concurrency,
            "requests": self.requests,
            "duration": self.duration,
            "rps": self.rps,
            "avg_latency": self.avg_latency,
            "median_latency": self.median_latency,
            "min_latency": self.min_latency,
            "max_latency": self.max_latency,
            "stdev_latency": self.stdev_latency,
            "errors": self.errors
        }

def make_request(url: str, method: str, path: str, data: Optional[Dict] = None,
                headers: Optional[Dict] = None) -> Tuple[float, bool]:
    """Make a single HTTP request and return the latency and success status."""
    full_url = f"{url}{path}"
    headers = headers or {}
    
    start_time = time.time()
    try:
        if method.upper() == "GET":
            response = requests.get(full_url, headers=headers, timeout=5)
        elif method.upper() == "POST":
            response = requests.post(full_url, json=data, headers=headers, timeout=5)
        else:
            raise ValueError(f"Unsupported HTTP method: {method}")
        
        success = 200 <= response.status_code < 300
    except Exception:
        success = False
    
    end_time = time.time()
    latency = (end_time - start_time) * 1000  # Convert to ms
    
    return latency, success

def run_benchmark(name: str, url: str, method: str, path: str, data: Optional[Dict],
                 headers: Optional[Dict], concurrency: int, duration: int) -> BenchmarkResult:
    """Run a benchmark for a specific endpoint."""
    console.print(f"Running benchmark: [bold]{name}[/bold] with concurrency {concurrency}...")
    
    start_time = time.time()
    end_time = start_time + duration
    
    latencies = []
    errors = 0
    requests_count = 0
    
    with ThreadPoolExecutor(max_workers=concurrency) as executor:
        futures = []
        
        for _ in range(concurrency):
            futures.append(executor.submit(
                make_request, url, method, path, data, headers
            ))
        
        while time.time() < end_time:
            for i, future in enumerate(futures):
                if future.done():
                    latency, success = future.result()
                    latencies.append(latency)
                    requests_count += 1
                    
                    if not success:
                        errors += 1
                    
                    if time.time() < end_time:
                        futures[i] = executor.submit(
                            make_request, url, method, path, data, headers
                        )
            
            time.sleep(0.01)  # Small sleep to prevent CPU spinning
    
    actual_duration = time.time() - start_time
    
    return BenchmarkResult(
        name=name,
        concurrency=concurrency,
        requests=requests_count,
        duration=actual_duration,
        latencies=latencies,
        errors=errors
    )

def print_results(python_results: List[BenchmarkResult], rust_results: List[BenchmarkResult]):
    """Print benchmark results in a table."""
    table = Table(title="Benchmark Results")
    
    table.add_column("Endpoint", style="cyan")
    table.add_column("Implementation", style="magenta")
    table.add_column("Concurrency", style="blue")
    table.add_column("Requests/s", style="green")
    table.add_column("Avg Latency (ms)", style="yellow")
    table.add_column("Median Latency (ms)", style="yellow")
    table.add_column("Max Latency (ms)", style="red")
    table.add_column("Errors", style="red")
    
    endpoints = set(r.name for r in python_results + rust_results)
    concurrencies = set(r.concurrency for r in python_results + rust_results)
    
    for endpoint in endpoints:
        for concurrency in sorted(concurrencies):
            py_result = next((r for r in python_results if r.name == endpoint and r.concurrency == concurrency), None)
            rust_result = next((r for r in rust_results if r.name == endpoint and r.concurrency == concurrency), None)
            
            if py_result:
                table.add_row(
                    endpoint,
                    "Python",
                    str(py_result.concurrency),
                    f"{py_result.rps:.2f}",
                    f"{py_result.avg_latency:.2f}",
                    f"{py_result.median_latency:.2f}",
                    f"{py_result.max_latency:.2f}",
                    str(py_result.errors)
                )
            
            if rust_result:
                table.add_row(
                    endpoint,
                    "Rust",
                    str(rust_result.concurrency),
                    f"{rust_result.rps:.2f}",
                    f"{rust_result.avg_latency:.2f}",
                    f"{rust_result.median_latency:.2f}",
                    f"{rust_result.max_latency:.2f}",
                    str(rust_result.errors)
                )
            
            if py_result and rust_result:
                speedup = rust_result.rps / py_result.rps if py_result.rps > 0 else float('inf')
                latency_improvement = py_result.avg_latency / rust_result.avg_latency if rust_result.avg_latency > 0 else float('inf')
                
                table.add_row(
                    "",
                    f"[bold]Rust vs Python[/bold]",
                    "",
                    f"[bold]{speedup:.2f}x[/bold]",
                    f"[bold]{latency_improvement:.2f}x[/bold]",
                    "",
                    "",
                    ""
                )
                
                table.add_row("", "", "", "", "", "", "", "")
    
    console.print(table)

def save_results(python_results: List[BenchmarkResult], rust_results: List[BenchmarkResult], 
                output_file: str):
    """Save benchmark results to a JSON file."""
    results = {
        "timestamp": datetime.now().isoformat(),
        "python": [r.to_dict() for r in python_results],
        "rust": [r.to_dict() for r in rust_results]
    }
    
    with open(output_file, 'w') as f:
        json.dump(results, f, indent=2)
    
    console.print(f"Results saved to [bold]{output_file}[/bold]")

def check_server(url: str) -> bool:
    """Check if a server is running at the given URL."""
    try:
        response = requests.get(f"{url}/", timeout=2)
        return response.status_code < 500
    except Exception:
        return False

def main():
    parser = argparse.ArgumentParser(description="Benchmark RequestRepo implementations")
    parser.add_argument("--python-url", default=DEFAULT_CONFIG["python_url"],
                        help="URL of the Python implementation")
    parser.add_argument("--rust-url", default=DEFAULT_CONFIG["rust_url"],
                        help="URL of the Rust implementation")
    parser.add_argument("--concurrency", type=int, nargs="+", default=DEFAULT_CONFIG["concurrency"],
                        help="Concurrency levels to test")
    parser.add_argument("--duration", type=int, default=DEFAULT_CONFIG["duration"],
                        help="Duration of each benchmark in seconds")
    parser.add_argument("--output", default="benchmark_results.json",
                        help="Output file for results")
    parser.add_argument("--config", help="Path to benchmark configuration file")
    parser.add_argument("--python-only", action="store_true", help="Only benchmark Python implementation")
    parser.add_argument("--rust-only", action="store_true", help="Only benchmark Rust implementation")
    
    args = parser.parse_args()
    
    config = DEFAULT_CONFIG
    if args.config:
        with open(args.config, 'r') as f:
            config = json.load(f)
    
    config["python_url"] = args.python_url
    config["rust_url"] = args.rust_url
    config["concurrency"] = args.concurrency
    config["duration"] = args.duration
    
    if not args.rust_only and not check_server(config["python_url"]):
        console.print(f"[bold red]Error:[/bold red] Python server not running at {config['python_url']}")
        console.print("Start the Python server with: make start-backend")
        if not args.python_only:
            console.print("Continuing with Rust benchmarks only...")
            args.python_only = False
            args.rust_only = True
        else:
            return 1
    
    if not args.python_only and not check_server(config["rust_url"]):
        console.print(f"[bold red]Error:[/bold red] Rust server not running at {config['rust_url']}")
        console.print("Start the Rust server with: cd src && cargo run --release")
        if not args.rust_only:
            console.print("Continuing with Python benchmarks only...")
            args.python_only = True
            args.rust_only = False
        else:
            return 1
    
    python_results = []
    rust_results = []
    
    for endpoint in config["endpoints"]:
        for concurrency in config["concurrency"]:
            if not args.rust_only:
                python_result = run_benchmark(
                    name=endpoint["name"],
                    url=config["python_url"],
                    method=endpoint["method"],
                    path=endpoint["path"],
                    data=endpoint.get("data"),
                    headers=endpoint.get("headers", {}),
                    concurrency=concurrency,
                    duration=config["duration"]
                )
                python_results.append(python_result)
            
            if not args.python_only:
                rust_result = run_benchmark(
                    name=endpoint["name"],
                    url=config["rust_url"],
                    method=endpoint["method"],
                    path=endpoint["path"],
                    data=endpoint.get("data"),
                    headers=endpoint.get("headers", {}),
                    concurrency=concurrency,
                    duration=config["duration"]
                )
                rust_results.append(rust_result)
    
    print_results(python_results, rust_results)
    save_results(python_results, rust_results, args.output)
    
    return 0

if __name__ == "__main__":
    sys.exit(main())
