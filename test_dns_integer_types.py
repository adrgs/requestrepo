import requests
import json
import sys

def test_dns_integer_types():
    print("Testing DNS record type handling with integer types...")
    
    # Get token
    headers = {"Content-Type": "application/json"}
    response = requests.post("http://localhost:8001/api/get_token", headers=headers, json={})
    data = response.json()
    token = data["token"]
    subdomain = data["subdomain"]
    print(f"Token: {token}")
    print(f"Subdomain: {subdomain}")
    
    # Test updating DNS with integer record types
    dns_records = {
        "records": [
            {"domain": "test1", "type": 0, "value": "1.1.1.1", "subdomain": subdomain},
            {"domain": "test2", "type": 1, "value": "2001:db8::1", "subdomain": subdomain},
            {"domain": "test3", "type": 2, "value": "example.com", "subdomain": subdomain},
            {"domain": "test4", "type": 3, "value": "v=spf1 include:_spf.example.com ~all", "subdomain": subdomain}
        ]
    }
    
    update_response = requests.post(
        f"http://localhost:8001/api/update_dns?token={token}",
        json=dns_records
    )
    
    print(f"Update response status: {update_response.status_code}")
    if update_response.status_code != 200:
        print(f"Error: {update_response.text}")
        return False
    
    # Get DNS records to verify
    get_response = requests.get(f"http://localhost:8001/api/get_dns?token={token}")
    if get_response.status_code != 200:
        print(f"Error getting DNS records: {get_response.text}")
        return False
    
    records = get_response.json()
    print("Retrieved DNS records:")
    print(json.dumps(records, indent=2))
    
    # Verify record types were correctly converted
    expected_types = ["A", "AAAA", "CNAME", "TXT"]
    
    # Handle both formats: direct list or records field
    records_list = records if isinstance(records, list) else records.get("records", [])
    
    if not records_list:
        print("Error: No DNS records found")
        return False
    
    for i, record in enumerate(records_list):
        if i >= len(expected_types):
            break
        if record["type"] != expected_types[i]:
            print(f"Error: Record {i} has type {record['type']}, expected {expected_types[i]}")
            return False
    
    print("All DNS record types were correctly handled!")
    return True

if __name__ == "__main__":
    success = test_dns_integer_types()
    sys.exit(0 if success else 1)
