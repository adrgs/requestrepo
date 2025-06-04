import requests
import json
import sys

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

dns_data = {
    "records": [
        {
            "domain": "test",
            "type": 0,  # Integer type (A record)
            "value": "1.1.1.1",
            "subdomain": subdomain
        }
    ]
}

print("\nTesting DNS update with integer type...")
response = requests.post(f"http://localhost:8001/api/update_dns?token={token}",
                        headers={"Content-Type": "application/json"},
                        data=json.dumps(dns_data))
print(f"Response status: {response.status_code}")
print(f"Response text: {response.text}")

print("\nVerifying DNS records...")
response = requests.get(f"http://localhost:8001/api/get_dns?token={token}")
print(f"Response status: {response.status_code}")
print(f"Response text: {response.text}")
