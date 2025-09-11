import requests
import json
import time
import sys

# Test header case preservation
def test_header_case_preservation():
    # Get a token
    response = requests.post("http://localhost:8001/api/get_token")
    token = response.json()["token"]
    subdomain = response.json()["subdomain"]
    
    # Send a request with custom headers to the subdomain
    custom_headers = {
        "X-Custom-Header": "TestValue",
        "Another-Custom-Header": "AnotherValue",
        "Content-Type": "application/json"
    }
    
    url = f"http://{subdomain}.localhost:8001/"
    response = requests.get(url, headers=custom_headers)
    
    # Get the logged request
    time.sleep(1)  # Wait for request to be logged
    response = requests.get(f"http://localhost:8001/api/requests?token={token}")
    requests_data = response.json()
    
    if not requests_data:
        print("No requests found")
        return False
    
    # Parse the first request
    request = json.loads(requests_data[0]) if isinstance(requests_data[0], str) else requests_data[0]
    
    # Check if headers preserved case
    headers = request.get("headers", {})
    print("Headers in logged request:", json.dumps(headers, indent=2))
    
    # Check if our custom headers are present with original case
    case_preserved = True
    for header_name, header_value in custom_headers.items():
        if header_name not in headers:
            print(f"Header '{header_name}' not found in logged request")
            case_preserved = False
    
    return case_preserved

if __name__ == "__main__":
    result = test_header_case_preservation()
    print(f"Header case preservation test {'PASSED' if result else 'FAILED'}")
    sys.exit(0 if result else 1)
