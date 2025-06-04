import requests
import json
import sys

def test_http_logging():
    print("Testing HTTP request logging format...")
    
    # Get token
    headers = {"Content-Type": "application/json"}
    response = requests.post("http://localhost:8001/api/get_token", headers=headers, json={})
    data = response.json()
    token = data["token"]
    subdomain = data["subdomain"]
    print(f"Token: {token}")
    print(f"Subdomain: {subdomain}")
    
    # Make a request to generate a log
    custom_headers = {
        "X-Custom-Header": "TestValue",
        "User-Agent": "TestAgent",
        "Accept": "application/json",
        "X-Another-Header": "AnotherValue"
    }
    
    test_url = f"http://{subdomain}.localhost:8001/test?param1=value1&param2=value2"
    response = requests.get(test_url, headers=custom_headers)
    print(f"Test request status: {response.status_code}")
    
    # Get the request log
    response = requests.get(f"http://localhost:8001/api/requests?token={token}")
    requests_data = response.json()
    
    if not requests_data:
        print("Error: No request logs found")
        return False
    
    # Print the first request log
    request_log = requests_data[0]
    print("\nRequest log:")
    print(json.dumps(request_log, indent=2))
    
    # Check for required fields
    required_fields = ["_id", "type", "raw", "uid", "method", "path", "headers", 
                      "date", "ip", "port", "protocol", "fragment", "query", "url"]
    
    missing_fields = [field for field in required_fields if field not in request_log]
    if missing_fields:
        print(f"Error: Missing fields in request log: {missing_fields}")
        return False
    
    # Check header case preservation
    headers_in_log = request_log["headers"]
    print("\nHeaders in log:")
    for header, value in headers_in_log.items():
        print(f"  {header}: {value}")
    
    # Check if custom headers are present with correct case
    for header, value in custom_headers.items():
        if header not in headers_in_log and header.lower() not in headers_in_log:
            print(f"Error: Header '{header}' not found in log")
            return False
    
    print("\nAll required fields present in request log!")
    return True

if __name__ == "__main__":
    success = test_http_logging()
    sys.exit(0 if success else 1)
