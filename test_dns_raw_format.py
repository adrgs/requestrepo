#!/usr/bin/env python3
import socket
import base64
import json
import time
import requests
import dns.message
import dns.rdatatype
import dns.rdataclass

response = requests.get("http://localhost:8001/api/token")
data = response.json()
token = data["token"]
subdomain = data["subdomain"]

query = dns.message.make_query(
    f"test.{subdomain}.localhost.", dns.rdatatype.A, dns.rdataclass.IN
)
wire_format = query.to_wire()

sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.sendto(wire_format, ("localhost", 5353))
response_data, _ = sock.recvfrom(4096)
sock.close()

time.sleep(1)

response = requests.get(f"http://localhost:8001/api/requests?token={token}")
requests_data = response.json()

dns_request = None
for req in requests_data:
    if req.get("type") == "dns":
        dns_request = req
        break

if dns_request:
    print("DNS Request found:")
    print(f"  ID: {dns_request.get('_id')}")
    print(f"  Type: {dns_request.get('type')}")
    print(f"  UID: {dns_request.get('uid')}")
    
    raw = dns_request.get("raw")
    try:
        decoded = base64.b64decode(raw)
        print(f"  Raw data (decoded length): {len(decoded)} bytes")
        print(f"  Original query length: {len(wire_format)} bytes")
        if len(decoded) == len(wire_format):
            print("  ✅ Raw data length matches original query")
        else:
            print("  ❌ Raw data length does not match original query")
    except Exception as e:
        print(f"  ❌ Failed to decode raw data: {e}")
    
    reply = dns_request.get("reply")
    if reply:
        print("  Reply field exists:")
        print(f"  {reply[:100]}...")
        if "HEADER" in reply and "QUESTION SECTION" in reply:
            print("  ✅ Reply field is formatted correctly")
        else:
            print("  ❌ Reply field is not formatted correctly")
    else:
        print("  ❌ Reply field is missing")
    
    country = dns_request.get("country")
    print(f"  Country: {country}")
    if country is not None:
        print("  ✅ Country field exists")
    else:
        print("  ❌ Country field is missing")
else:
    print("❌ No DNS request found")
