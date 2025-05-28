#!/bin/bash

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${GREEN}RequestRepo Benchmark Tool${NC}"
echo "Comparing Python FastAPI vs Rust implementation"
echo "----------------------------------------------"

if ! command -v wrk &> /dev/null; then
    echo -e "${RED}Error: wrk is not installed. Please install it first.${NC}"
    echo "On Ubuntu: sudo apt-get install -y wrk"
    exit 1
fi

if [ ! -d "../backend" ]; then
    echo -e "${RED}Error: Python backend directory not found.${NC}"
    exit 1
fi

if [ ! -d "../src" ]; then
    echo -e "${RED}Error: Rust src directory not found.${NC}"
    exit 1
fi

mkdir -p results

start_python_server() {
    echo -e "${YELLOW}Starting Python FastAPI server...${NC}"
    cd ..
    python -m backend.app > benchmarks/results/python_server.log 2>&1 &
    PYTHON_PID=$!
    echo "Python server started with PID: $PYTHON_PID"
    sleep 3
    cd benchmarks
}

start_rust_server() {
    echo -e "${YELLOW}Starting Rust server...${NC}"
    cd ../src
    cargo run --release > ../benchmarks/results/rust_server.log 2>&1 &
    RUST_PID=$!
    echo "Rust server started with PID: $RUST_PID"
    sleep 3
    cd ../benchmarks
}

stop_servers() {
    echo -e "${YELLOW}Stopping servers...${NC}"
    if [ ! -z "$PYTHON_PID" ]; then
        kill $PYTHON_PID 2>/dev/null || true
    fi
    if [ ! -z "$RUST_PID" ]; then
        kill $RUST_PID 2>/dev/null || true
    fi
    pkill -f "python -m backend.app" 2>/dev/null || true
    pkill -f "target/release/requestrepo" 2>/dev/null || true
    sleep 2
}

trap stop_servers EXIT

DURATION=30s
CONNECTIONS=(10 50 100 200)
THREADS=4
ENDPOINTS=(
    "http://localhost:8000/api/token"
    "http://localhost:8000/api/dns"
)

echo -e "${GREEN}Running Python FastAPI benchmarks...${NC}"
start_python_server

for endpoint in "${ENDPOINTS[@]}"; do
    endpoint_name=$(echo $endpoint | awk -F/ '{print $NF}')
    echo -e "${YELLOW}Testing endpoint: $endpoint_name${NC}"
    
    for conn in "${CONNECTIONS[@]}"; do
        echo "Running with $conn connections..."
        wrk -t$THREADS -c$conn -d$DURATION $endpoint > results/python_${endpoint_name}_c${conn}.txt
        
        REQUESTS=$(grep "Requests/sec" results/python_${endpoint_name}_c${conn}.txt | awk '{print $2}')
        LATENCY=$(grep "Latency" results/python_${endpoint_name}_c${conn}.txt | awk '{print $2}')
        TRANSFER=$(grep "Transfer/sec" results/python_${endpoint_name}_c${conn}.txt | awk '{print $2}')
        
        echo "  Requests/sec: $REQUESTS"
        echo "  Latency: $LATENCY"
        echo "  Transfer/sec: $TRANSFER"
    done
done

stop_servers
sleep 5

echo -e "${GREEN}Running Rust implementation benchmarks...${NC}"
start_rust_server

for endpoint in "${ENDPOINTS[@]}"; do
    endpoint_name=$(echo $endpoint | awk -F/ '{print $NF}')
    echo -e "${YELLOW}Testing endpoint: $endpoint_name${NC}"
    
    for conn in "${CONNECTIONS[@]}"; do
        echo "Running with $conn connections..."
        wrk -t$THREADS -c$conn -d$DURATION $endpoint > results/rust_${endpoint_name}_c${conn}.txt
        
        REQUESTS=$(grep "Requests/sec" results/rust_${endpoint_name}_c${conn}.txt | awk '{print $2}')
        LATENCY=$(grep "Latency" results/rust_${endpoint_name}_c${conn}.txt | awk '{print $2}')
        TRANSFER=$(grep "Transfer/sec" results/rust_${endpoint_name}_c${conn}.txt | awk '{print $2}')
        
        echo "  Requests/sec: $REQUESTS"
        echo "  Latency: $LATENCY"
        echo "  Transfer/sec: $TRANSFER"
    done
done

echo -e "${GREEN}Generating benchmark summary...${NC}"
python3 benchmark.py

echo -e "${GREEN}Benchmarks completed. Results are in the 'results' directory and summary.md${NC}"
