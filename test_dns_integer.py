import requests
import json
import sys

def test_dns_integer_types():
    print("Testing DNS integer record types...")
    
    response = requests.post("http://localhost:8001/api/get_token", 
                            headers={"Content-Type": "application/json"},
                            json={})
    data = response.json()
    token = data["token"]
    subdomain = data["subdomain"]
    print(f"Token: {token}")
    print(f"Subdomain: {subdomain}")
    
    dns_records = {
        "records": [
            {
                "domain": "test1",
                "type": 0,  # A record as integer (0)
                "value": "1.1.1.1",
                "subdomain": subdomain
            },
            {
                "domain": "test2",
                "type": 1,  # AAAA record as integer (1)
                "value": "2001:db8::1",
                "subdomain": subdomain
            },
            {
                "domain": "test3",
                "type": 2,  # CNAME record as integer (2)
                "value": "example.com",
                "subdomain": subdomain
            },
            {
                "domain": "test4",
                "type": 3,  # TXT record as integer (3)
                "value": "v=spf1 -all",
                "subdomain": subdomain
            }
        ]
    }
    
    response = requests.post(f"http://localhost:8001/api/update_dns?token={token}", json=dns_records)
    print(f"DNS update response: {response.status_code}")
    print(response.text)
    
    if response.status_code != 200:
        print("Error: Failed to update DNS records")
        return False
    
    response = requests.get(f"http://localhost:8001/api/get_dns?token={token}")
    dns_data = response.json()
    
    print("\nDNS records:")
    print(json.dumps(dns_data, indent=2))
    
    if len(dns_data) != 4:
        print(f"Error: Expected 4 DNS records, got {len(dns_data)}")
        return False
    
    type_mapping = {0: "A", 1: "AAAA", 2: "CNAME", 3: "TXT"}
    for record in dns_data:
        domain = record.get("domain")
        record_type = record.get("type")
        
        if "test" in domain:
            test_part = domain.split('.')[0]
            if test_part.startswith("test") and test_part[4:].isdigit():
                test_num = int(test_part[4:])
                expected_type = type_mapping.get(test_num - 1)
                
                if record_type != expected_type:
                    print(f"Error: Record type mismatch for {domain}. Expected {expected_type}, got {record_type}")
                    return False
    
    print("\nAll DNS integer record types handled correctly!")
    return True

if __name__ == "__main__":
    success = test_dns_integer_types()
    sys.exit(0 if success else 1)
