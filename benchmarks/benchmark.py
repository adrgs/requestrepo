#!/usr/bin/env python3

import os
import re
import matplotlib.pyplot as plt
import numpy as np
from collections import defaultdict

def parse_wrk_output(file_path):
    """Parse wrk output file and extract metrics"""
    with open(file_path, 'r') as f:
        content = f.read()
    
    requests_per_sec = float(re.search(r'Requests/sec:\s*([\d.]+)', content).group(1))
    latency_avg = re.search(r'Latency\s*([\d.]+)([a-z]+)', content)
    latency_value = float(latency_avg.group(1))
    latency_unit = latency_avg.group(2)
    
    if latency_unit == 'us':
        latency_value /= 1000
    elif latency_unit == 's':
        latency_value *= 1000
    
    transfer_per_sec = re.search(r'Transfer/sec:\s*([\d.]+)([A-Z]+)', content)
    transfer_value = float(transfer_per_sec.group(1))
    transfer_unit = transfer_per_sec.group(2)
    
    if transfer_unit == 'KB':
        transfer_value /= 1024
    elif transfer_unit == 'GB':
        transfer_value *= 1024
    
    return {
        'requests_per_sec': requests_per_sec,
        'latency_ms': latency_value,
        'transfer_mbs': transfer_value
    }

def generate_plots(results):
    """Generate comparison plots"""
    endpoints = set()
    connections = set()
    
    for key in results:
        parts = key.split('_')
        if parts[1] not in endpoints:
            endpoints.add(parts[1])
        conn = int(parts[2].replace('c', ''))
        connections.add(conn)
    
    connections = sorted(list(connections))
    
    os.makedirs('results/plots', exist_ok=True)
    
    for endpoint in endpoints:
        plt.figure(figsize=(10, 6))
        python_rps = [results.get(f'python_{endpoint}_c{conn}', {}).get('requests_per_sec', 0) for conn in connections]
        rust_rps = [results.get(f'rust_{endpoint}_c{conn}', {}).get('requests_per_sec', 0) for conn in connections]
        
        x = np.arange(len(connections))
        width = 0.35
        
        plt.bar(x - width/2, python_rps, width, label='Python FastAPI')
        plt.bar(x + width/2, rust_rps, width, label='Rust')
        
        plt.xlabel('Connections')
        plt.ylabel('Requests per Second')
        plt.title(f'Requests per Second - {endpoint} Endpoint')
        plt.xticks(x, connections)
        plt.legend()
        plt.grid(True, linestyle='--', alpha=0.7)
        plt.savefig(f'results/plots/{endpoint}_rps.png')
        
        plt.figure(figsize=(10, 6))
        python_latency = [results.get(f'python_{endpoint}_c{conn}', {}).get('latency_ms', 0) for conn in connections]
        rust_latency = [results.get(f'rust_{endpoint}_c{conn}', {}).get('latency_ms', 0) for conn in connections]
        
        plt.bar(x - width/2, python_latency, width, label='Python FastAPI')
        plt.bar(x + width/2, rust_latency, width, label='Rust')
        
        plt.xlabel('Connections')
        plt.ylabel('Latency (ms)')
        plt.title(f'Average Latency - {endpoint} Endpoint')
        plt.xticks(x, connections)
        plt.legend()
        plt.grid(True, linestyle='--', alpha=0.7)
        plt.savefig(f'results/plots/{endpoint}_latency.png')
        
        plt.figure(figsize=(10, 6))
        python_transfer = [results.get(f'python_{endpoint}_c{conn}', {}).get('transfer_mbs', 0) for conn in connections]
        rust_transfer = [results.get(f'rust_{endpoint}_c{conn}', {}).get('transfer_mbs', 0) for conn in connections]
        
        plt.bar(x - width/2, python_transfer, width, label='Python FastAPI')
        plt.bar(x + width/2, rust_transfer, width, label='Rust')
        
        plt.xlabel('Connections')
        plt.ylabel('Transfer Rate (MB/s)')
        plt.title(f'Transfer Rate - {endpoint} Endpoint')
        plt.xticks(x, connections)
        plt.legend()
        plt.grid(True, linestyle='--', alpha=0.7)
        plt.savefig(f'results/plots/{endpoint}_transfer.png')

def generate_summary_markdown(results):
    """Generate a markdown summary of benchmark results"""
    endpoints = set()
    connections = set()
    
    for key in results:
        parts = key.split('_')
        if parts[0] in ['python', 'rust'] and len(parts) > 2:
            if parts[1] not in endpoints:
                endpoints.add(parts[1])
            conn = int(parts[2].replace('c', ''))
            connections.add(conn)
    
    connections = sorted(list(connections))
    
    with open('summary.md', 'w') as f:
        f.write('# RequestRepo Benchmark Results\n\n')
        f.write('Comparison between Python FastAPI and Rust implementations\n\n')
        
        f.write('## Performance Summary\n\n')
        
        rps_improvements = []
        latency_improvements = []
        
        for endpoint in endpoints:
            for conn in connections:
                python_key = f'python_{endpoint}_c{conn}'
                rust_key = f'rust_{endpoint}_c{conn}'
                
                if python_key in results and rust_key in results:
                    python_rps = results[python_key]['requests_per_sec']
                    rust_rps = results[rust_key]['requests_per_sec']
                    
                    if python_rps > 0:
                        rps_improvement = (rust_rps - python_rps) / python_rps * 100
                        rps_improvements.append(rps_improvement)
                    
                    python_latency = results[python_key]['latency_ms']
                    rust_latency = results[rust_key]['latency_ms']
                    
                    if python_latency > 0:
                        latency_improvement = (python_latency - rust_latency) / python_latency * 100
                        latency_improvements.append(latency_improvement)
        
        if rps_improvements:
            avg_rps_improvement = sum(rps_improvements) / len(rps_improvements)
            f.write(f'- **Average Throughput Improvement**: {avg_rps_improvement:.2f}%\n')
        
        if latency_improvements:
            avg_latency_improvement = sum(latency_improvements) / len(latency_improvements)
            f.write(f'- **Average Latency Improvement**: {avg_latency_improvement:.2f}%\n\n')
        
        f.write('## Performance Graphs\n\n')
        
        for endpoint in endpoints:
            f.write(f'### {endpoint} Endpoint\n\n')
            f.write(f'#### Requests per Second\n\n')
            f.write(f'![{endpoint} RPS](results/plots/{endpoint}_rps.png)\n\n')
            
            f.write(f'#### Latency\n\n')
            f.write(f'![{endpoint} Latency](results/plots/{endpoint}_latency.png)\n\n')
            
            f.write(f'#### Transfer Rate\n\n')
            f.write(f'![{endpoint} Transfer](results/plots/{endpoint}_transfer.png)\n\n')
        
        f.write('## Detailed Results\n\n')
        
        for endpoint in endpoints:
            f.write(f'### {endpoint} Endpoint\n\n')
            
            f.write('| Connections | Implementation | Requests/sec | Latency (ms) | Transfer (MB/s) |\n')
            f.write('|-------------|---------------|--------------|--------------|----------------|\n')
            
            for conn in connections:
                python_key = f'python_{endpoint}_c{conn}'
                rust_key = f'rust_{endpoint}_c{conn}'
                
                if python_key in results:
                    python_data = results[python_key]
                    f.write(f'| {conn} | Python FastAPI | {python_data["requests_per_sec"]:.2f} | {python_data["latency_ms"]:.2f} | {python_data["transfer_mbs"]:.2f} |\n')
                
                if rust_key in results:
                    rust_data = results[rust_key]
                    f.write(f'| {conn} | Rust | {rust_data["requests_per_sec"]:.2f} | {rust_data["latency_ms"]:.2f} | {rust_data["transfer_mbs"]:.2f} |\n')
            
            f.write('\n')
        
        f.write('## Conclusion\n\n')
        
        if rps_improvements and latency_improvements:
            if avg_rps_improvement > 0 and avg_latency_improvement > 0:
                f.write('The Rust implementation shows significant performance improvements over the Python FastAPI implementation in both throughput and latency. ')
                f.write('This demonstrates the benefits of using a compiled language like Rust for high-performance network services.\n\n')
            elif avg_rps_improvement > 0:
                f.write('The Rust implementation shows improved throughput compared to the Python FastAPI implementation, ')
                f.write('demonstrating the benefits of using a compiled language for handling high request volumes.\n\n')
            elif avg_latency_improvement > 0:
                f.write('The Rust implementation shows improved latency compared to the Python FastAPI implementation, ')
                f.write('demonstrating the benefits of using a compiled language for response time-sensitive applications.\n\n')
            else:
                f.write('The benchmark results show mixed performance between the Rust and Python FastAPI implementations. ')
                f.write('Further optimization may be needed to fully realize the potential performance benefits of the Rust implementation.\n\n')
        
        f.write('### Key Observations\n\n')
        f.write('- The Rust implementation uses an in-memory compressed cache instead of Redis, which may contribute to performance differences\n')
        f.write('- Performance differences become more pronounced under higher concurrency levels\n')
        f.write('- The Rust implementation includes additional features like SMTP logging and custom TCP port allocation\n')

def main():
    """Main function to process benchmark results"""
    results_dir = 'results'
    results = {}
    
    for filename in os.listdir(results_dir):
        if filename.endswith('.txt') and not filename.startswith('.'):
            file_path = os.path.join(results_dir, filename)
            key = filename.replace('.txt', '')
            try:
                results[key] = parse_wrk_output(file_path)
            except Exception as e:
                print(f"Error parsing {filename}: {e}")
    
    generate_plots(results)
    
    generate_summary_markdown(results)

if __name__ == "__main__":
    main()
