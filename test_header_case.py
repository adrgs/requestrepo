import requests
import json
import sys
import time

response = requests.post("http://localhost:8001/api/get_token", 
                        headers={"Content-Type": "application/json"}, 
                        data="{}")
print(f"Response status: {response.status_code}")
print(f"Response text: {response.text}")

data = response.json()
token = data["token"]
subdomain = data["subdomain"]
print(f"Token: {token}")
print(f"Subdomain: {subdomain}")

url = f"http://{subdomain}.localhost:8001/"
headers = {
    "User-Agent": "TestClient/1.0",
    "X-Custom-Header": "PreserveThisCase",
    "Content-Type": "application/json",
    "Accept-Language": "en-US,en;q=0.9",
    "Cache-Control": "no-cache"
}
response = requests.get(url, headers=headers)
print(f"Response status: {response.status_code}")

time.sleep(1)

response = requests.get(f"http://localhost:8001/api/requests?token={token}")
print(f"Response status: {response.status_code}")

if response.status_code == 200:
    data = response.json()
    print(f"Found {len(data)} requests")
    
    if len(data) > 0:
        request = data[0]
        print("\nRequest details:")
        print(json.dumps(request, indent=2))
        
        required_fields = ["_id", "type", "raw", "uid", "method", "path", "headers", 
                          "date", "ip", "country", "port", "protocol", "fragment", "query", "url"]
        missing_fields = [field for field in required_fields if field not in request]
        
        if missing_fields:
            print(f"\nMissing fields: {missing_fields}")
        else:
            print("\nAll required fields are present!")
        
        if "headers" in request:
            print("\nDetailed header analysis:")
            print("Original headers sent:")
            for name, value in headers.items():
                print(f"  {name}: {value}")
            
            print("\nHeaders received in response:")
            for name, value in request["headers"].items():
                print(f"  {name}: {value}")
            
            print("\nCase-insensitive matching:")
            for sent_name in headers.keys():
                found = False
                for received_name in request["headers"].keys():
                    if sent_name.lower() == received_name.lower():
                        found = True
                        print(f"  ✅ {sent_name} -> {received_name}")
                        break
                if not found:
                    print(f"  ❌ {sent_name} not found in any form")
        else:
            print("\nNo headers found in response")
    else:
        print("\nNo requests found")
else:
    print(f"Error getting requests: {response.text}")
