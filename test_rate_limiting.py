#!/usr/bin/env python3
import requests
import time

print("Testing rate limiting for token generation...")
print("Attempting to generate 15 tokens in quick succession")

success_count = 0
rate_limited_count = 0

for i in range(15):
    response = requests.get("http://localhost:8001/api/token")
    if response.status_code == 200:
        success_count += 1
        print(f"✅ Token {i+1}: Generated successfully")
    elif response.status_code == 429:
        rate_limited_count += 1
        print(f"🚫 Token {i+1}: Rate limited ({response.json().get('detail')})")
    else:
        print(f"❌ Token {i+1}: Unexpected status code {response.status_code}")
    
    time.sleep(0.1)

print(f"\nResults: {success_count} tokens generated, {rate_limited_count} requests rate limited")
if rate_limited_count > 0:
    print("✅ Rate limiting is working correctly")
else:
    print("❌ Rate limiting is not working")

print("\nWaiting 60 seconds for rate limit to reset...")
time.sleep(60)

print("Trying again after rate limit reset")
response = requests.get("http://localhost:8001/api/token")
if response.status_code == 200:
    print("✅ Token generated successfully after waiting")
else:
    print(f"❌ Failed to generate token after waiting: {response.status_code}")
