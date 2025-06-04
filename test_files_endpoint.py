import requests
import json
import sys

def test_files_endpoint():
    # Get a token
    response = requests.post("http://localhost:8001/api/get_token")
    token = response.json()["token"]
    subdomain = response.json()["subdomain"]
    
    # Test /api/files endpoint
    response = requests.get(f"http://localhost:8001/api/files?token={token}")
    
    if response.status_code != 200:
        print(f"Failed to get files: {response.status_code}")
        return False
    
    files = response.json()
    print("Files response:", json.dumps(files, indent=2))
    
    # Check if index.html exists (should be created by write_basic_file)
    if "index.html" not in files:
        print("index.html not found in files response")
        return False
    
    # Try to access the index.html file
    response = requests.get(f"http://{subdomain}.localhost:8001/")
    
    if response.status_code != 200:
        print(f"Failed to access index.html: {response.status_code}")
        return False
    
    print("Successfully accessed index.html")
    return True

if __name__ == "__main__":
    result = test_files_endpoint()
    print(f"Files endpoint test {'PASSED' if result else 'FAILED'}")
    sys.exit(0 if result else 1)
