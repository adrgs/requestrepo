"""Integration tests for requestrepo library against requestrepo.com.

These tests run against the live requestrepo.com instance.
"""

import base64
import threading
import time

import pytest
import requests

import sys
sys.path.insert(0, "src")

from requestrepo import (
    Requestrepo,
    HttpRequest,
    DnsRequest,
    DnsRecord,
    Header,
    Response,
)


class TestSessionCreation:
    """Tests for session creation."""

    def test_create_session(self) -> None:
        """Test creating a new session."""
        repo = Requestrepo()

        assert repo.subdomain is not None
        assert len(repo.subdomain) > 0
        assert repo.domain == "requestrepo.com"
        assert repo.token is not None
        assert len(repo.token) > 0

    def test_create_session_with_existing_token(self) -> None:
        """Test creating a client with an existing token."""
        repo1 = Requestrepo()
        token = repo1.token
        subdomain = repo1.subdomain

        repo2 = Requestrepo(token=token)

        assert repo2.token == token
        assert repo2.subdomain == subdomain


class TestDnsOperations:
    """Tests for DNS record operations."""

    @pytest.fixture
    def repo(self) -> Requestrepo:
        """Create a fresh session for each test."""
        return Requestrepo()

    def test_get_dns_records(self, repo: Requestrepo) -> None:
        """Test getting DNS records."""
        records = repo.dns()

        assert isinstance(records, list)

    def test_add_dns_record(self, repo: Requestrepo) -> None:
        """Test adding a DNS record."""
        result = repo.add_dns("test", "A", "1.2.3.4")

        assert result is True

        records = repo.dns()
        assert any(
            r.domain == "test" and r.type == "A" and r.value == "1.2.3.4"
            for r in records
        )

    def test_update_dns_records(self, repo: Requestrepo) -> None:
        """Test updating DNS records."""
        new_records = [
            DnsRecord(type="A", domain="www", value="10.0.0.1"),
            DnsRecord(type="AAAA", domain="ipv6", value="::1"),
        ]

        result = repo.update_dns(new_records)

        assert result is True

        records = repo.dns()
        assert len(records) == 2

    def test_remove_dns_record(self, repo: Requestrepo) -> None:
        """Test removing a DNS record."""
        repo.add_dns("to-remove", "A", "1.1.1.1")
        repo.add_dns("to-keep", "A", "2.2.2.2")

        result = repo.remove_dns("to-remove")

        assert result is True

        records = repo.dns()
        assert not any(r.domain == "to-remove" for r in records)
        assert any(r.domain == "to-keep" for r in records)


class TestFileOperations:
    """Tests for file/response operations."""

    @pytest.fixture
    def repo(self) -> Requestrepo:
        """Create a fresh session for each test."""
        return Requestrepo()

    def test_get_files(self, repo: Requestrepo) -> None:
        """Test getting all files."""
        files = repo.files()

        assert isinstance(files, dict)

    def test_set_file(self, repo: Requestrepo) -> None:
        """Test setting a file."""
        result = repo.set_file(
            path="test.txt",
            body="Hello, World!",
            status_code=200,
            headers=[Header(header="X-Custom", value="test")],
        )

        assert result is True

        files = repo.files()
        assert "test.txt" in files
        assert files["test.txt"].status_code == 200

    def test_set_file_with_bytes(self, repo: Requestrepo) -> None:
        """Test setting a file with bytes content."""
        content = b"\x00\x01\x02\x03"
        result = repo.set_file(path="binary.bin", body=content)

        assert result is True

        files = repo.files()
        assert "binary.bin" in files
        decoded = base64.b64decode(files["binary.bin"].raw)
        assert decoded == content

    def test_update_files(self, repo: Requestrepo) -> None:
        """Test updating all files."""
        new_files = {
            "index.html": Response(
                raw=base64.b64encode(b"<html></html>").decode(),
                headers=[Header(header="Content-Type", value="text/html")],
                status_code=200,
            ),
            "api/data.json": Response(
                raw=base64.b64encode(b'{"key": "value"}').decode(),
                headers=[Header(header="Content-Type", value="application/json")],
                status_code=200,
            ),
        }

        result = repo.update_files(new_files)

        assert result is True

        files = repo.files()
        assert "index.html" in files
        assert "api/data.json" in files


class TestRequestOperations:
    """Tests for request capture operations."""

    @pytest.fixture
    def repo(self) -> Requestrepo:
        """Create a fresh session for each test."""
        return Requestrepo()

    def test_list_requests_empty(self, repo: Requestrepo) -> None:
        """Test listing requests on a new session."""
        reqs = repo.list_requests()

        assert isinstance(reqs, list)
        # New session should have no requests
        assert len(reqs) == 0

    def test_delete_all_requests(self, repo: Requestrepo) -> None:
        """Test deleting all requests."""
        result = repo.delete_all_requests()

        assert result is True

    def test_capture_http_request(self, repo: Requestrepo) -> None:
        """Test capturing an HTTP request."""
        # Make a request to the subdomain
        url = f"https://{repo.subdomain}.{repo.domain}/test-path?foo=bar"
        try:
            requests.get(url, timeout=5, headers={"X-Test": "integration"})
        except Exception:
            pass  # May fail due to custom response, that's ok

        # Wait a bit for the request to be captured
        time.sleep(1)

        # Check if request was captured
        reqs = repo.list_requests()

        assert len(reqs) >= 1
        http_reqs = [r for r in reqs if isinstance(r, HttpRequest)]
        assert len(http_reqs) >= 1

        # Verify request details
        req = http_reqs[0]
        assert req.method == "GET"
        assert "/test-path" in req.path
        assert req.ip is not None

    def test_delete_single_request(self, repo: Requestrepo) -> None:
        """Test deleting a single request."""
        # Make a request
        url = f"https://{repo.subdomain}.{repo.domain}/to-delete"
        try:
            requests.get(url, timeout=5)
        except Exception:
            pass

        time.sleep(1)

        reqs = repo.list_requests()
        if len(reqs) > 0:
            req_id = reqs[0].id
            result = repo.delete_request(req_id)

            assert result is True


class TestRequestSharing:
    """Tests for request sharing functionality."""

    @pytest.fixture
    def repo(self) -> Requestrepo:
        """Create a fresh session for each test."""
        return Requestrepo()

    def test_share_and_get_request(self, repo: Requestrepo) -> None:
        """Test sharing a request and retrieving it."""
        # Make a request
        url = f"https://{repo.subdomain}.{repo.domain}/shared-test"
        try:
            requests.get(url, timeout=5)
        except Exception:
            pass

        time.sleep(1)

        reqs = repo.list_requests()
        if len(reqs) > 0:
            req_id = reqs[0].id

            # Share the request
            share_token = repo.share_request(req_id)

            assert share_token is not None
            assert len(share_token) > 0

            # Get shared request (no auth required)
            shared_req = repo.get_shared_request(share_token)

            assert shared_req is not None
            assert shared_req.id == req_id


class TestWebSocket:
    """Tests for WebSocket real-time functionality."""

    def test_websocket_connection(self) -> None:
        """Test WebSocket connection and historical requests."""
        repo = Requestrepo()

        # Start WebSocket in background
        repo.start_requests()

        try:
            # Make a request
            url = f"https://{repo.subdomain}.{repo.domain}/ws-test"
            try:
                requests.get(url, timeout=5)
            except Exception:
                pass

            # Wait for request to arrive
            time.sleep(2)

            # Should be able to ping
            assert repo.ping() is True
        finally:
            repo.stop_requests()

    def test_websocket_receive_request(self) -> None:
        """Test receiving a request via WebSocket."""
        received_requests: list = []

        class TestRepo(Requestrepo):
            def on_request(self, req):
                received_requests.append(req)

        repo = TestRepo()
        repo.start_requests()

        try:
            # Make a request
            url = f"https://{repo.subdomain}.{repo.domain}/ws-receive-test"
            try:
                requests.get(url, timeout=5)
            except Exception:
                pass

            # Wait for request to arrive
            time.sleep(2)

            assert len(received_requests) >= 1
            assert isinstance(received_requests[0], HttpRequest)
        finally:
            repo.stop_requests()


class TestRequestFilters:
    """Tests for request filter functions."""

    def test_http_filter(self) -> None:
        """Test HTTP filter function."""
        from requestrepo.models import HttpRequest, DnsRequest

        http_req = HttpRequest(
            _id="1", type="http", raw=b"", uid="u", ip="1.1.1.1",
            date=0, method="GET", path="/", http_version="HTTP/1.1", headers={}
        )
        dns_req = DnsRequest(
            _id="2", type="dns", raw=b"", uid="u", ip="1.1.1.1",
            date=0, query_type="A", domain="test.com"
        )

        assert Requestrepo.HTTP_FILTER(http_req) is True
        assert Requestrepo.HTTP_FILTER(dns_req) is False

    def test_dns_filter(self) -> None:
        """Test DNS filter function."""
        from requestrepo.models import HttpRequest, DnsRequest

        http_req = HttpRequest(
            _id="1", type="http", raw=b"", uid="u", ip="1.1.1.1",
            date=0, method="GET", path="/", http_version="HTTP/1.1", headers={}
        )
        dns_req = DnsRequest(
            _id="2", type="dns", raw=b"", uid="u", ip="1.1.1.1",
            date=0, query_type="A", domain="test.com"
        )

        assert Requestrepo.DNS_FILTER(http_req) is False
        assert Requestrepo.DNS_FILTER(dns_req) is True


class TestPagination:
    """Tests for request pagination."""

    @pytest.fixture
    def repo(self) -> Requestrepo:
        """Create a fresh session for each test."""
        return Requestrepo()

    def test_list_requests_with_limit(self, repo: Requestrepo) -> None:
        """Test listing requests with a limit."""
        # Make multiple requests
        for i in range(3):
            url = f"https://{repo.subdomain}.{repo.domain}/pagination-{i}"
            try:
                requests.get(url, timeout=5)
            except Exception:
                pass

        time.sleep(2)

        # Get with limit
        reqs = repo.list_requests(limit=2)

        assert len(reqs) <= 2

    def test_list_requests_with_offset(self, repo: Requestrepo) -> None:
        """Test listing requests with an offset."""
        # Make multiple requests
        for i in range(3):
            url = f"https://{repo.subdomain}.{repo.domain}/offset-{i}"
            try:
                requests.get(url, timeout=5)
            except Exception:
                pass

        time.sleep(2)

        all_reqs = repo.list_requests()
        offset_reqs = repo.list_requests(offset=1)

        # Offset should return fewer results
        if len(all_reqs) > 1:
            assert len(offset_reqs) == len(all_reqs) - 1


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
