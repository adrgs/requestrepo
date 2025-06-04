import time
import requests
import statistics
import sys

def test_dns_performance():
    # Get a token
    response = requests.post("http://localhost:8001/api/get_token")
    token = response.json()["token"]
    subdomain = response.json()["subdomain"]
    
    print(f"Testing DNS performance for subdomain: {subdomain}")
    
    # Create DNS records
    dns_records = [
        {"domain": "test", "type": "A", "value": "1.1.1.1", "subdomain": subdomain}
    ]
    
    response = requests.post(
        f"http://localhost:8001/api/update_dns?token={token}",
        json={"records": dns_records}
    )
    
    if response.status_code != 200:
        print(f"Failed to update DNS records: {response.status_code}")
        return False
    
    # Benchmark DNS query performance
    num_requests = 100
    times = []
    
    for i in range(num_requests):
        start_time = time.time()
        response = requests.get(f"http://localhost:8001/api/get_dns?token={token}")
        end_time = time.time()
        
        if response.status_code != 200:
            print(f"Failed to get DNS records: {response.status_code}")
            return False
        
        times.append(end_time - start_time)
    
    avg_time = statistics.mean(times)
    median_time = statistics.median(times)
    min_time = min(times)
    max_time = max(times)
    
    print(f"DNS Performance Results (seconds):")
    print(f"  Average: {avg_time:.6f}")
    print(f"  Median:  {median_time:.6f}")
    print(f"  Min:     {min_time:.6f}")
    print(f"  Max:     {max_time:.6f}")
    
    return True

if __name__ == "__main__":
    result = test_dns_performance()
    print(f"DNS performance test {'PASSED' if result else 'FAILED'}")
    sys.exit(0 if result else 1)
