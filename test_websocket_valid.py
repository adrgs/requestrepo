import asyncio
import json
import websockets
import requests
import sys

async def test_websocket_v2_with_valid_token():
    print("Testing WebSocket v2 endpoint with valid token...")
    
    # First get a valid token
    headers = {"Content-Type": "application/json"}
    response = requests.post("http://localhost:8001/api/get_token", headers=headers, json={})
    data = response.json()
    token = data["token"]
    subdomain = data["subdomain"]
    print(f"Token: {token}")
    print(f"Subdomain: {subdomain}")
    
    # Make a test HTTP request to generate a log
    test_url = f"http://{subdomain}.localhost:8001/test?param=value"
    response = requests.get(test_url, headers={"X-Test-Header": "TestValue"})
    print(f"Test request status: {response.status_code}")
    
    # Connect to WebSocket v2 endpoint
    uri = "ws://localhost:8001/api/ws2"
    async with websockets.connect(uri) as websocket:
        print("Connected to WebSocket v2")
        
        # Send token message
        token_msg = json.dumps({"cmd": "connect", "token": token})
        await websocket.send(token_msg)
        
        # Receive response (should be connected for valid token)
        response = await websocket.recv()
        print(f"Response to connect: {response}")
        response_data = json.loads(response)
        
        if response_data.get("cmd") != "connected":
            print("Error: Failed to connect with valid token")
            return False
        
        # Send requests message to get logs
        requests_msg = json.dumps({"cmd": "requests"})
        await websocket.send(requests_msg)
        
        # Receive requests response
        response = await websocket.recv()
        print(f"Response to requests: {response}")
        response_data = json.loads(response)
        
        if response_data.get("cmd") != "requests" or "data" not in response_data:
            print("Error: Invalid response to requests command")
            return False
            
        # Check if we got the test request in the logs
        if not response_data.get("data"):
            print("Error: No requests found in response")
            return False
            
        # Check the format of the request data
        request_data = response_data["data"][0]
        print("\nRequest data format:")
        for key, value in request_data.items():
            print(f"  {key}: {type(value).__name__}")
            
        # Check for required fields
        required_fields = ["_id", "type", "raw", "uid", "ip", "port", "headers", 
                          "method", "protocol", "path", "fragment", "query", "url", "date"]
        missing_fields = [field for field in required_fields if field not in request_data]
        
        if missing_fields:
            print(f"Error: Missing required fields: {missing_fields}")
            return False
            
        print("\nWebSocket v2 functionality working correctly!")
        return True

async def main():
    try:
        success = await test_websocket_v2_with_valid_token()
        return 0 if success else 1
    except Exception as e:
        print(f"Error: {e}")
        return 1

if __name__ == "__main__":
    exit_code = asyncio.run(main())
    sys.exit(exit_code)
