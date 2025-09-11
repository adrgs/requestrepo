import asyncio
import json
import sys
import websockets

async def test_websocket_v2():
    print("Testing WebSocket v2 functionality...")
    
    import requests
    response = requests.post("http://localhost:8001/api/get_token", 
                            headers={"Content-Type": "application/json"},
                            json={})
    data = response.json()
    token = data["token"]
    subdomain = data["subdomain"]
    print(f"Token: {token}")
    print(f"Subdomain: {subdomain}")
    
    uri = f"ws://localhost:8001/api/ws2"
    async with websockets.connect(uri) as websocket:
        print("Connected to WebSocket v2")
        
        await websocket.send(json.dumps({"cmd": "connect", "token": token}))
        response = await websocket.recv()
        print(f"Connect response: {response}")
        
        response_data = json.loads(response)
        if response_data.get("cmd") != "connected":
            print(f"Error: Expected 'connected' response, got {response_data.get('cmd')}")
            return False
        
        await websocket.send(json.dumps({"cmd": "ping"}))
        response = await websocket.recv()
        print(f"Ping response: {response}")
        
        response_data = json.loads(response)
        if response_data.get("cmd") != "pong":
            print(f"Error: Expected 'pong' response, got {response_data.get('cmd')}")
            return False
        
        await websocket.send(json.dumps({"cmd": "requests"}))
        response = await websocket.recv()
        print(f"Requests response: {response}")
        
        response_data = json.loads(response)
        if response_data.get("cmd") != "requests":
            print(f"Error: Expected 'requests' response, got {response_data.get('cmd')}")
            return False
        
        if "data" not in response_data:
            print("Error: No 'data' field in requests response")
            return False
        
        if "subdomain" not in response_data:
            print("Error: No 'subdomain' field in requests response")
            return False
        
        if response_data.get("subdomain") != subdomain:
            print(f"Error: Expected subdomain {subdomain}, got {response_data.get('subdomain')}")
            return False
        
        print("\nWebSocket v2 functionality working correctly!")
        return True

if __name__ == "__main__":
    asyncio.run(test_websocket_v2())
