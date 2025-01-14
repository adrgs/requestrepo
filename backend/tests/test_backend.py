import pytest
from fastapi.testclient import TestClient
from backend.app import app, redis_dependency
from backend.utils import (
    get_random_subdomain,
    get_subdomain_from_hostname,
    get_subdomain_from_path,
    verify_jwt
)
import jwt
import datetime
from unittest.mock import patch, AsyncMock
import json

# Setup mock Redis before creating test client
mock_redis = AsyncMock()
mock_redis.exists.return_value = False
mock_redis.get.return_value = None
mock_redis.set.return_value = True
mock_redis.delete.return_value = True
mock_redis.rpush.return_value = 1
mock_redis.lrange.return_value = []
mock_redis.lset.return_value = True
mock_redis.publish.return_value = 1

# Create a proper async mock pubsub
class AsyncPubSubMock(AsyncMock):
    def __call__(self):  # This makes the mock callable and returns self
        return self

    async def subscribe(self, *args, **kwargs):
        return True

    async def unsubscribe(self, *args, **kwargs):
        return True

    def listen(self):
        async def message_generator():
            yield {"type": "message", "data": json.dumps({"test": "data"})}
        return message_generator()

# Setup mock Redis before creating test client
mock_redis = AsyncMock()
mock_redis.exists.return_value = False
mock_redis.get.return_value = None
mock_redis.set.return_value = True
mock_redis.delete.return_value = True
mock_redis.rpush.return_value = 1
mock_redis.lrange.return_value = []
mock_redis.lset.return_value = True
mock_redis.publish.return_value = 1

# Create a pubsub instance and set it as the return value for pubsub()
mock_pubsub = AsyncPubSubMock()
mock_redis.pubsub = mock_pubsub  # This makes pubsub() return the mock_pubsub instance

# Create an async function that returns our mock redis
async def override_get_redis():
    return mock_redis

@pytest.fixture(autouse=True)
def override_dependencies():
    # Override the redis dependency
    app.dependency_overrides[redis_dependency.get_redis] = override_get_redis
    yield
    app.dependency_overrides.clear()

# Create test client after setting up dependencies
client = TestClient(app)

def test_get_subdomain_from_hostname():
    # Test valid subdomains
    assert get_subdomain_from_hostname("abcd1234.localhost") == "abcd1234"
    assert get_subdomain_from_hostname("test.abcd1234.localhost") == "abcd1234"
    assert get_subdomain_from_hostname("longabcd1234.localhost") == "abcd1234"
    
    # Test invalid subdomains
    assert get_subdomain_from_hostname("invalid#.localhost") is None
    assert get_subdomain_from_hostname("localhost") is None

def test_get_subdomain_from_path():
    # Test valid paths
    assert get_subdomain_from_path("/r/abcd1234") == "abcd1234"
    assert get_subdomain_from_path("/r/abcd1234/") == "abcd1234"
    assert get_subdomain_from_path("/r/toolong12345") == "toolong1"
    
    # Test invalid paths
    assert get_subdomain_from_path("/short") is None
    assert get_subdomain_from_path("/r/short?q=query") is None
    assert get_subdomain_from_path("/r/short#asdf1234") is None

def test_get_random_subdomain():
    subdomain = get_random_subdomain()
    assert len(subdomain) == 8
    assert subdomain.isalnum()

def test_get_token():
    response = client.post("/api/get_token")
    assert response.status_code == 200
    
    data = response.json()
    assert "token" in data
    assert "subdomain" in data
    
    # Verify the token is valid
    decoded = verify_jwt(data["token"])
    assert decoded == data["subdomain"]

def test_update_dns():
    # Create a valid token
    payload = {
        "iat": datetime.datetime.utcnow(),
        "exp": datetime.datetime.utcnow() + datetime.timedelta(days=1),
        "subdomain": "abcd1234"
    }
    token = jwt.encode(payload, "test-secret", algorithm="HS256")
    
    dns_records = {
        "records": [
            {"domain": "test", "type": 0, "value": "1.2.3.4"}
        ]
    }
    
    with patch('backend.app.config.jwt_secret', "test-secret"):
        response = client.post(
            "/api/update_dns",
            params={"token": token},
            json=dns_records
        )
        
        assert response.status_code == 200
        assert response.json() == {"msg": "Updated records"}

def test_invalid_token():
    response = client.post(
        "/api/update_dns",
        params={"token": "invalid-token"},
        json={"records": []}
    )
    assert response.status_code == 403
    assert response.json() == {"detail": "Invalid token"}

def test_delete_request():
    # Create a valid token
    payload = {
        "iat": datetime.datetime.utcnow(),
        "exp": datetime.datetime.utcnow() + datetime.timedelta(days=1),
        "subdomain": "abcd1234"
    }
    token = jwt.encode(payload, "test-secret", algorithm="HS256")
    
    with patch('backend.app.config.jwt_secret', "test-secret"):
        response = client.post(
            "/api/delete_request",
            params={"token": token},
            json={"id": "test-id"}
        )
        
        assert response.status_code == 200
        assert response.json() == {"msg": "Deleted request"}

def test_create_and_view_request():
    # First create a token
    payload = {
        "iat": datetime.datetime.utcnow(),
        "exp": datetime.datetime.utcnow() + datetime.timedelta(days=1),
        "subdomain": "abcd1234"
    }
    token = jwt.encode(payload, "test-secret", algorithm="HS256")

    # Mock the request that will be stored
    request_data = {
        "_id": "test-request-id",
        "type": "http",
        "raw": "SGVsbG8gV29ybGQ=",  # base64 encoded "Hello World"
        "uid": "abcd1234",
        "method": "GET",
        "path": "/test",
        "headers": {"host": "test.com"},
        "date": int(datetime.datetime.now(datetime.timezone.utc).timestamp())
    }
    mock_redis.lrange.return_value = [json.dumps(request_data)]
    
    with patch('backend.app.config.jwt_secret', "test-secret"):
        # Get requests for the subdomain
        response = client.get("/api/get_dns", params={"token": token})
        assert response.status_code == 200
        
        # Delete the specific request
        response = client.post(
            "/api/delete_request",
            params={"token": token},
            json={"id": "test-request-id"}
        )
        assert response.status_code == 200
        assert response.json() == {"msg": "Deleted request"}

def test_update_and_retrieve_file():
    payload = {
        "iat": datetime.datetime.utcnow(),
        "exp": datetime.datetime.utcnow() + datetime.timedelta(days=1),
        "subdomain": "abcd1234"
    }
    token = jwt.encode(payload, "test-secret", algorithm="HS256")

    # Create custom response file
    file_data = {
        "raw": "SGVsbG8gV29ybGQ=",  # base64 encoded "Hello World"
        "headers": [
            {"header": "Content-Type", "value": "text/plain"},
            {"header": "X-Custom", "value": "test"}
        ],
        "status_code": 200
    }

    # Mock Redis get to return our file
    mock_redis.get.return_value = json.dumps(file_data)

    with patch('backend.app.config.jwt_secret', "test-secret"):
        # Update the file
        response = client.post(
            "/api/update_file",
            params={"token": token},
            json=file_data
        )
        assert response.status_code == 200
        assert response.json() == {"msg": "Updated response"}

        # Retrieve the file
        response = client.get("/api/get_file", params={"token": token})
        assert response.status_code == 200
        assert json.loads(response.content) == file_data

def test_update_and_retrieve_dns():
    payload = {
        "iat": datetime.datetime.utcnow(),
        "exp": datetime.datetime.utcnow() + datetime.timedelta(days=1),
        "subdomain": "abcd1234"
    }
    token = jwt.encode(payload, "test-secret", algorithm="HS256")

    dns_records = {
        "records": [
            {"domain": "test", "type": 0, "value": "1.2.3.4"},
            {"domain": "www", "type": 2, "value": "example.com"}
        ]
    }

    # Mock Redis get to return our DNS records
    stored_records = [
        {"domain": "test.abcd1234.localhost.", "type": "A", "value": "1.2.3.4"},
        {"domain": "www.abcd1234.localhost.", "type": "CNAME", "value": "example.com"}
    ]
    mock_redis.get.side_effect = lambda key: (
        json.dumps(stored_records) if key == "dns:abcd1234" else None
    )

    with patch('backend.app.config.jwt_secret', "test-secret"):
        # Update DNS records
        response = client.post(
            "/api/update_dns",
            params={"token": token},
            json=dns_records
        )
        assert response.status_code == 200
        assert response.json() == {"msg": "Updated records"}

        # Retrieve DNS records
        response = client.get("/api/get_dns", params={"token": token})
        assert response.status_code == 200
        assert response.json() == stored_records

def test_websocket_connection():
    payload = {
        "iat": datetime.datetime.utcnow(),
        "exp": datetime.datetime.utcnow() + datetime.timedelta(days=1),
        "subdomain": "abcd1234"
    }
    token = jwt.encode(payload, "test-secret", algorithm="HS256")

    with patch('backend.app.config.jwt_secret', "test-secret"):
        with client.websocket_connect("/api/ws") as websocket:
            # Send token
            websocket.send_text(token)
            
            # Should receive initial requests data
            data = websocket.receive_json()
            assert data["cmd"] == "requests"
            
            # Should receive published message
            data = websocket.receive_json()
            assert data["cmd"] == "request"
            assert "data" in data

def test_catch_all_endpoint():
    file_data = {
        "raw": "SGVsbG8gV29ybGQ=",  # base64 encoded "Hello World"
        "headers": [
            {"header": "Content-Type", "value": "text/plain"},
            {"header": "X-Custom", "value": "test"}
        ],
        "status_code": 200
    }

    # Reset and configure mock Redis
    mock_redis.get.reset_mock()
    mock_redis.get.side_effect = None
    mock_redis.get.return_value = json.dumps(file_data)

    # Test subdomain request
    with patch('backend.app.config.jwt_secret', "test-secret"):
        response = client.get(
            "/", 
            headers={"host": f"abcd1234.localhost"}
        )
        assert response.status_code == 200
        assert response.headers["Content-Type"] == "text/plain"
        assert response.headers["X-Custom"] == "test"
        assert response.content == b"Hello World"