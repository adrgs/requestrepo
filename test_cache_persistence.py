#!/usr/bin/env python3
import requests
import json
import time
import os
import subprocess

response = requests.get("http://localhost:8001/api/token")
data = response.json()
token = data["token"]
subdomain = data["subdomain"]

print(f"Testing cache persistence with subdomain: {subdomain}")

file_content = "This is a test file for cache persistence"
files_data = {
    "test_persistence.txt": file_content
}

response = requests.post(
    f"http://localhost:8001/api/files?token={token}",
    json=files_data
)

print(f"Created test file, status code: {response.status_code}")

response = requests.get(f"http://localhost:8001/api/files?token={token}")
files = response.json()

if "test_persistence.txt" in files:
    print("✅ File created successfully")
else:
    print("❌ File not created")
    exit(1)

print("\nStopping the server...")
subprocess.run(["pkill", "-f", "target/debug/requestrepo"])
time.sleep(2)

print("Starting the server again...")
os.environ["CACHE_PERSISTENCE_PATH"] = "/tmp/requestrepo_cache.json"
subprocess.Popen(
    ["cargo", "run", "--bin", "requestrepo"],
    cwd="/home/ubuntu/repos/requestrepo/src",
    env=os.environ
)

time.sleep(5)

print("\nChecking if the file still exists after restart...")
response = requests.get(f"http://localhost:8001/api/files?token={token}")

if response.status_code == 200:
    files = response.json()
    if "test_persistence.txt" in files:
        print("✅ File persisted after server restart")
    else:
        print("❌ File not found after server restart")
else:
    print(f"❌ Failed to get files: {response.status_code}")
