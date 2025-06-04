import requests
import json
import sys

def test_token_endpoint():
    print("Testing token endpoint...")
    try:
        headers = {"Content-Type": "application/json"}
        response = requests.post("http://localhost:8001/api/get_token", headers=headers, json={})
        print(f"Status code: {response.status_code}")
        print(f"Response text: {response.text}")
        if response.status_code == 200:
            data = response.json()
            print(f"Token: {data['token']}")
            print(f"Subdomain: {data['subdomain']}")
            return True
        return False
    except Exception as e:
        print(f"Error: {e}")
        return False

if __name__ == "__main__":
    success = test_token_endpoint()
    sys.exit(0 if success else 1)
