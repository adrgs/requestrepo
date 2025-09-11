import socket
import time
import requests
import json
import sys

def test_smtp_subdomain_extraction():
    print("Testing SMTP subdomain extraction...")
    
    # First get a valid token
    headers = {"Content-Type": "application/json"}
    response = requests.post("http://localhost:8001/api/get_token", headers=headers, json={})
    data = response.json()
    token = data["token"]
    subdomain = data["subdomain"]
    print(f"Token: {token}")
    print(f"Subdomain: {subdomain}")
    
    # Connect to SMTP server
    smtp_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    smtp_socket.connect(('localhost', 2525))  # Using test port 2525 instead of 25
    
    # Read welcome message
    welcome = smtp_socket.recv(1024).decode('utf-8')
    print(f"SMTP Welcome: {welcome}")
    
    # Send HELO
    smtp_socket.send(b'HELO example.com\r\n')
    response = smtp_socket.recv(1024).decode('utf-8')
    print(f"HELO response: {response}")
    
    # Send MAIL FROM
    smtp_socket.send(b'MAIL FROM:<test@example.com>\r\n')
    response = smtp_socket.recv(1024).decode('utf-8')
    print(f"MAIL FROM response: {response}")
    
    # Send RCPT TO with the subdomain
    rcpt_cmd = f'RCPT TO:<test@{subdomain}.localhost>\r\n'
    smtp_socket.send(rcpt_cmd.encode('utf-8'))
    response = smtp_socket.recv(1024).decode('utf-8')
    print(f"RCPT TO response: {response}")
    
    # Send DATA
    smtp_socket.send(b'DATA\r\n')
    response = smtp_socket.recv(1024).decode('utf-8')
    print(f"DATA response: {response}")
    
    # Send email content
    email_content = f"Subject: Test Email\r\n\r\nThis is a test email to {subdomain}.\r\n.\r\n"
    smtp_socket.send(email_content.encode('utf-8'))
    response = smtp_socket.recv(1024).decode('utf-8')
    print(f"Email content response: {response}")
    
    # Send QUIT
    smtp_socket.send(b'QUIT\r\n')
    response = smtp_socket.recv(1024).decode('utf-8')
    print(f"QUIT response: {response}")
    
    smtp_socket.close()
    
    # Wait a moment for the server to process the email
    time.sleep(1)
    
    # Check if the email was logged correctly
    headers = {"Authorization": f"Bearer {token}"}
    response = requests.get(f"http://localhost:8001/api/requests?token={token}", headers=headers)
    requests_data = response.json()
    
    # Find SMTP requests in the logs
    smtp_requests = [req for req in requests_data if req.get("type") == "smtp"]
    
    if not smtp_requests:
        print("Error: No SMTP requests found in logs")
        return False
        
    print("\nSMTP requests found in logs:")
    for req in smtp_requests:
        print(json.dumps(req, indent=2))
        
    # Verify the subdomain was extracted correctly
    if smtp_requests[0].get("uid") == subdomain:
        print(f"\nSuccess! SMTP request was correctly logged to subdomain: {subdomain}")
        return True
    else:
        print(f"\nError: SMTP request was logged to incorrect subdomain: {smtp_requests[0].get('uid')}")
        return False

if __name__ == "__main__":
    success = test_smtp_subdomain_extraction()
    sys.exit(0 if success else 1)
