import asyncio
import websockets
import json
import requests
import time
import sys

async def test_websocket_v2_realtime():
    # Get a token
    response = requests.post("http://localhost:8001/api/get_token")
    token = response.json()["token"]
    subdomain = response.json()["subdomain"]
    
    # Connect to WebSocket v2
    async with websockets.connect("ws://localhost:8001/api/ws2") as websocket:
        # Send connect command with token
        await websocket.send(json.dumps({
            "cmd": "connect",
            "token": token
        }))
        
        # Wait for connected response
        response = await websocket.recv()
        response_data = json.loads(response)
        print("Connected response:", response_data)
        
        if response_data.get("cmd") != "connected":
            print("Failed to connect to WebSocket v2")
            return False
        
        # Send a ping to verify connection
        await websocket.send(json.dumps({"cmd": "ping"}))
        response = await websocket.recv()
        response_data = json.loads(response)
        print("Ping response:", response_data)
        
        if response_data.get("cmd") != "pong":
            print("Failed to receive pong response")
            return False
        
        # Start a task to listen for real-time updates
        real_time_updates = []
        
        async def listen_for_updates():
            try:
                while True:
                    response = await asyncio.wait_for(websocket.recv(), timeout=5)
                    data = json.loads(response)
                    if data.get("cmd") == "request":
                        real_time_updates.append(data)
                        print("Received real-time update:", data)
                        return True
            except asyncio.TimeoutError:
                print("Timeout waiting for real-time updates")
                return False
        
        listen_task = asyncio.create_task(listen_for_updates())
        
        # Make an HTTP request to trigger a real-time update
        time.sleep(1)
        print(f"Making HTTP request to {subdomain}.localhost:8001")
        requests.get(f"http://{subdomain}.localhost:8001/test-realtime")
        
        # Wait for the listen task to complete
        try:
            result = await asyncio.wait_for(listen_task, timeout=5)
            return result and len(real_time_updates) > 0
        except asyncio.TimeoutError:
            print("Timeout waiting for listen task to complete")
            return False

if __name__ == "__main__":
    result = asyncio.run(test_websocket_v2_realtime())
    print(f"WebSocket v2 real-time streaming test {'PASSED' if result else 'FAILED'}")
    sys.exit(0 if result else 1)
