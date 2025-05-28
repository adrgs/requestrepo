#!/bin/bash

set -e

PYTHON_PORT=21337
RUST_PORT=21338
DURATION=10
CONCURRENCY="1 5 10 50 100"
OUTPUT_FILE="benchmark_results.json"

while [[ $# -gt 0 ]]; do
  case $1 in
    --python-port)
      PYTHON_PORT="$2"
      shift 2
      ;;
    --rust-port)
      RUST_PORT="$2"
      shift 2
      ;;
    --duration)
      DURATION="$2"
      shift 2
      ;;
    --concurrency)
      CONCURRENCY="$2"
      shift 2
      ;;
    --output)
      OUTPUT_FILE="$2"
      shift 2
      ;;
    *)
      echo "Unknown option: $1"
      exit 1
      ;;
  esac
done

echo "Checking if Python server is running on port $PYTHON_PORT..."
if ! curl -s "http://localhost:$PYTHON_PORT/" > /dev/null; then
  echo "Starting Python server on port $PYTHON_PORT..."
  cd "$(dirname "$0")/.."
  HTTP_PORT=$PYTHON_PORT make start-backend &
  PYTHON_PID=$!
  
  echo "Waiting for Python server to start..."
  for i in {1..30}; do
    if curl -s "http://localhost:$PYTHON_PORT/" > /dev/null; then
      break
    fi
    sleep 1
    if [ $i -eq 30 ]; then
      echo "Failed to start Python server"
      exit 1
    fi
  done
  
  echo "Python server started with PID $PYTHON_PID"
else
  echo "Python server is already running"
  PYTHON_PID=""
fi

echo "Checking if Rust server is running on port $RUST_PORT..."
if ! curl -s "http://localhost:$RUST_PORT/" > /dev/null; then
  echo "Starting Rust server on port $RUST_PORT..."
  cd "$(dirname "$0")/../src"
  HTTP_PORT=$RUST_PORT cargo run --release &
  RUST_PID=$!
  
  echo "Waiting for Rust server to start..."
  for i in {1..30}; do
    if curl -s "http://localhost:$RUST_PORT/" > /dev/null; then
      break
    fi
    sleep 1
    if [ $i -eq 30 ]; then
      echo "Failed to start Rust server"
      [ -n "$PYTHON_PID" ] && kill $PYTHON_PID
      exit 1
    fi
  done
  
  echo "Rust server started with PID $RUST_PID"
else
  echo "Rust server is already running"
  RUST_PID=""
fi

echo "Running benchmark..."
cd "$(dirname "$0")"
python benchmark.py \
  --python-url "http://localhost:$PYTHON_PORT" \
  --rust-url "http://localhost:$RUST_PORT" \
  --duration "$DURATION" \
  --concurrency $CONCURRENCY \
  --output "$OUTPUT_FILE"

if [ -n "$PYTHON_PID" ]; then
  echo "Stopping Python server (PID $PYTHON_PID)..."
  kill $PYTHON_PID
fi

if [ -n "$RUST_PID" ]; then
  echo "Stopping Rust server (PID $RUST_PID)..."
  kill $RUST_PID
fi

echo "Benchmark completed. Results saved to $OUTPUT_FILE"
