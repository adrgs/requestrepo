import pytest
from utils import (
    verify_subdomain,
    verify_jwt,
    get_random_subdomain,
    get_subdomain_from_path,
    get_subdomain_from_hostname,
    write_basic_file,
)
import jwt
from unittest.mock import AsyncMock, patch
import json
from config import config


def test_verify_subdomain():
    assert verify_subdomain("abcd1234") is True
    assert verify_subdomain("12345678") is True
    assert verify_subdomain("abcdefgh") is True

    assert verify_subdomain("short") is False  # Too short
    assert verify_subdomain("toolong123456") is False  # Too long
    assert verify_subdomain("invalid#$") is False  # Invalid characters


def test_verify_jwt_valid_token():
    subdomain = "abcd1234"
    token = jwt.encode({"subdomain": subdomain}, config.jwt_secret, algorithm="HS256")

    result = verify_jwt(token)
    assert result == subdomain


def test_verify_jwt_invalid_token():
    assert verify_jwt("invalid-token") is None

    subdomain = "abcd1234"
    token = jwt.encode({"subdomain": subdomain}, "wrong-secret", algorithm="HS256")
    assert verify_jwt(token) is None

    token = jwt.encode({"other": "value"}, config.jwt_secret, algorithm="HS256")
    assert verify_jwt(token) is None

    token = jwt.encode({"subdomain": "invalid#"}, config.jwt_secret, algorithm="HS256")
    assert verify_jwt(token) is None


def test_get_random_subdomain():
    subdomain1 = get_random_subdomain()
    assert len(subdomain1) == config.subdomain_length
    assert all(c in config.subdomain_alphabet for c in subdomain1)

    custom_alphabet = "ABC123"
    custom_length = 4
    subdomain2 = get_random_subdomain(custom_alphabet, custom_length)
    assert len(subdomain2) == custom_length
    assert all(c in custom_alphabet for c in subdomain2)

    subdomain3 = get_random_subdomain()
    assert (
        subdomain1 != subdomain3
    )  # This could theoretically fail but is extremely unlikely


def test_get_subdomain_from_path_edge_cases():
    assert get_subdomain_from_path("") is None
    assert get_subdomain_from_path("/r/") is None
    assert get_subdomain_from_path("/r") is None
    assert get_subdomain_from_path("/R/abcd1234") == "abcd1234"  # Case insensitivity
    assert get_subdomain_from_path("//r//abcd1234") == "abcd1234"  # Extra slashes


def test_get_subdomain_from_hostname_edge_cases():
    assert get_subdomain_from_hostname("") is None
    assert get_subdomain_from_hostname("just.localhost") is None
    assert (
        get_subdomain_from_hostname("ABCD1234.localhost") == "abcd1234"
    )  # Case insensitivity

    custom_domain = "example.com"
    custom_length = 4
    assert (
        get_subdomain_from_hostname("abcd.example.com", custom_domain, custom_length)
        == "abcd"
    )
    assert (
        get_subdomain_from_hostname(
            "test.abcd.example.com", custom_domain, custom_length
        )
        == "abcd"
    )


@pytest.mark.asyncio
async def test_write_basic_file():
    mock_redis = AsyncMock()
    mock_redis.set.return_value = True

    subdomain = "abcd1234"
    await write_basic_file(subdomain, mock_redis)

    mock_redis.set.assert_called_once()
    call_args = mock_redis.set.call_args[0]
    assert call_args[0] == f"files:{subdomain}"

    file_data = json.loads(call_args[1])
    assert "index.html" in file_data
    assert file_data["index.html"]["status_code"] == 200
    assert isinstance(file_data["index.html"]["headers"], list)
    assert len(file_data["index.html"]["headers"]) >= 2  # At least 2 default headers

    with patch("config.config.include_server_domain", True):
        with patch("config.config.server_domain", "example.com"):
            await write_basic_file(subdomain, mock_redis)
            call_args = mock_redis.set.call_args[0]
            file_data = json.loads(call_args[1])
            headers = file_data["index.html"]["headers"]
            server_headers = [h for h in headers if h["header"] == "Server"]
            assert len(server_headers) == 1
            assert server_headers[0]["value"] == "example.com"
