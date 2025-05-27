import pytest
from fastapi.testclient import TestClient
from app import app, redis_dependency
from unittest.mock import patch
import json
import jwt
import uuid
import datetime
from backend.tests.test_backend import mock_redis, override_get_redis


@pytest.fixture(autouse=True)
def override_dependencies():
    app.dependency_overrides[redis_dependency.get_redis] = override_get_redis
    yield
    app.dependency_overrides.clear()


client = TestClient(app)


@pytest.mark.asyncio
async def test_get_files_endpoint():
    payload = {
        "iat": datetime.datetime.utcnow(),
        "exp": datetime.datetime.utcnow() + datetime.timedelta(days=1),
        "subdomain": "abcd1234",
    }
    token = jwt.encode(payload, "test-secret", algorithm="HS256")

    file_data = {
        "index.html": {
            "raw": "SGVsbG8gV29ybGQ=",  # base64 encoded "Hello World"
            "headers": [
                {"header": "Content-Type", "value": "text/plain"},
                {"header": "X-Custom", "value": "test"},
            ],
            "status_code": 200,
        },
        "test.js": {
            "raw": "Y29uc29sZS5sb2coJ0hlbGxvJyk7",  # base64 encoded "console.log('Hello');"
            "headers": [
                {"header": "Content-Type", "value": "application/javascript"},
            ],
            "status_code": 200,
        },
    }

    mock_redis.get.return_value = json.dumps(file_data)

    with patch("backend.app.config.jwt_secret", "test-secret"):
        response = client.get("/api/files", params={"token": token})

        assert response.status_code == 200
        assert response.json() == file_data

        mock_redis.get.assert_called_with("files:abcd1234")


@pytest.mark.asyncio
async def test_update_files_endpoint():
    payload = {
        "iat": datetime.datetime.utcnow(),
        "exp": datetime.datetime.utcnow() + datetime.timedelta(days=1),
        "subdomain": "abcd1234",
    }
    token = jwt.encode(payload, "test-secret", algorithm="HS256")

    file_data = {
        "index.html": {
            "raw": "SGVsbG8gV29ybGQ=",  # base64 encoded "Hello World"
            "headers": [
                {"header": "Content-Type", "value": "text/plain"},
                {"header": "X-Custom", "value": "test"},
            ],
            "status_code": 200,
        }
    }

    with patch("backend.app.config.jwt_secret", "test-secret"):
        response = client.post("/api/files", params={"token": token}, json=file_data)

        assert response.status_code == 200

        mock_redis.set.assert_called_with("files:abcd1234", json.dumps(file_data))


@pytest.mark.asyncio
async def test_get_request_endpoint():
    request_id = str(uuid.uuid4())
    subdomain = "abcd1234"

    request_data = {
        "_id": request_id,
        "type": "http",
        "raw": "SGVsbG8gV29ybGQ=",  # base64 encoded "Hello World"
        "uid": subdomain,
        "method": "GET",
        "path": "/test",
        "headers": {"host": "test.com"},
        "date": int(datetime.datetime.now(datetime.timezone.utc).timestamp()),
    }

    mock_redis.get.reset_mock()
    mock_redis.lindex.reset_mock()

    mock_redis.get.return_value = "0"  # Index in the list

    mock_redis.lindex.return_value = json.dumps(request_data)

    response = client.get(f"/api/get_request?id={request_id}&subdomain={subdomain}")

    assert response.status_code == 200
    assert response.json() == request_data

    mock_redis.get.assert_called_with(f"request:{subdomain}:{request_id}")
    mock_redis.lindex.assert_called_with(f"requests:{subdomain}", "0")


@pytest.mark.asyncio
async def test_get_request_invalid_id():
    request_id = "nonexistent-id"  # Not a valid UUID format
    subdomain = "abcd1234"

    mock_redis.get.reset_mock()

    response = client.get(f"/api/get_request?id={request_id}&subdomain={subdomain}")

    assert response.status_code == 404
    assert "detail" in response.json()
    assert "Invalid request ID" in response.json()["detail"]


@pytest.mark.asyncio
async def test_get_request_not_found():
    request_id = str(uuid.uuid4())
    subdomain = "abcd1234"

    mock_redis.get.reset_mock()
    mock_redis.get.return_value = None  # Request index not found

    response = client.get(f"/api/get_request?id={request_id}&subdomain={subdomain}")

    assert response.status_code == 404
    assert "detail" in response.json()
    assert "Request not found" in response.json()["detail"]


@pytest.mark.asyncio
async def test_websocket_invalid_token():
    with client.websocket_connect("/api/ws") as websocket:
        websocket.send_text("invalid-token")

        data = websocket.receive_json()
        assert "cmd" in data
        assert data["cmd"] == "invalid_token"
        assert "token" in data
        assert data["token"] == "invalid-token"


@pytest.mark.asyncio
async def test_delete_all_endpoint():
    payload = {
        "iat": datetime.datetime.utcnow(),
        "exp": datetime.datetime.utcnow() + datetime.timedelta(days=1),
        "subdomain": "abcd1234",
    }
    token = jwt.encode(payload, "test-secret", algorithm="HS256")

    mock_redis.delete.reset_mock()
    mock_redis.delete.return_value = True
    mock_redis.keys.reset_mock()
    mock_redis.keys.return_value = [
        "requests:abcd1234",
        "request:abcd1234:id1",
        "request:abcd1234:id2",
    ]
    mock_redis.lrange.reset_mock()
    mock_redis.lrange.return_value = []

    with patch("backend.app.config.jwt_secret", "test-secret"):
        response = client.post("/api/delete_all", params={"token": token})

        assert response.status_code == 200
        assert response.json() == {"msg": "Deleted all requests"}

        mock_redis.delete.assert_called()
