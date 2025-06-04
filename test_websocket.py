import asyncio
import json
import websockets
import sys

async def test_websocket_v2():
    print("Testing WebSocket v2 endpoint...")
    
    # Connect to WebSocket v2 endpoint
    uri = "ws://localhost:8001/api/ws2"
    async with websockets.connect(uri) as websocket:
        print("Connected to WebSocket v2")
        
        # Send token message
        token_msg = json.dumps({"cmd": "connect", "token": "invalid_token"})
        await websocket.send(token_msg)
        
        # Receive response (should be error for invalid token)
        response = await websocket.recv()
        print(f"Response to connect: {response}")
        
        # Send ping message
        ping_msg = json.dumps({"cmd": "ping"})
        await websocket.send(ping_msg)
        
        # Receive pong response
        response = await websocket.recv()
        print(f"Response to ping: {response}")
        
        # Try to get requests (should fail with invalid token)
        requests_msg = json.dumps({"cmd": "requests"})
        await websocket.send(requests_msg)
        
        # Receive error response
        response = await websocket.recv()
        print(f"Response to requests: {response}")
        
        return True

async def main():
    try:
        success = await test_websocket_v2()
        return 0 if success else 1
    except Exception as e:
        print(f"Error: {e}")
        return 1

if __name__ == "__main__":
    exit_code = asyncio.run(main())
    sys.exit(exit_code)
