#!/usr/bin/env python3
"""Integration tests for requestrepo library against requestrepo.com."""

import base64
import sys
import time
import traceback

sys.path.insert(0, "src")

import requests as http_requests

from requestrepo import (
    Requestrepo,
    HttpRequest,
    DnsRequest,
    DnsRecord,
    Header,
    Response,
)


class TestRunner:
    def __init__(self):
        self.passed = 0
        self.failed = 0
        self.errors = []

    def run_test(self, name: str, func):
        print(f"  {name}...", end=" ", flush=True)
        try:
            func()
            print("✓")
            self.passed += 1
        except AssertionError as e:
            print(f"✗ FAILED: {e}")
            self.failed += 1
            self.errors.append((name, str(e)))
        except Exception as e:
            print(f"✗ ERROR: {e}")
            self.failed += 1
            self.errors.append((name, traceback.format_exc()))

    def summary(self):
        print()
        print("=" * 60)
        print(f"Results: {self.passed} passed, {self.failed} failed")
        if self.errors:
            print("\nFailures:")
            for name, err in self.errors:
                print(f"  - {name}: {err[:200]}")
        print("=" * 60)
        return self.failed == 0


# Global session to avoid rate limiting
SHARED_REPO: Requestrepo | None = None


def get_repo() -> Requestrepo:
    """Get or create a shared session."""
    global SHARED_REPO
    if SHARED_REPO is None:
        SHARED_REPO = Requestrepo()
    return SHARED_REPO


def test_create_session():
    repo = get_repo()
    assert repo.subdomain is not None and len(repo.subdomain) > 0, "No subdomain"
    assert repo.domain == "requestrepo.com", f"Wrong domain: {repo.domain}"
    assert repo.token is not None and len(repo.token) > 0, "No token"


def test_create_session_with_token():
    repo1 = get_repo()
    repo2 = Requestrepo(token=repo1.token)
    assert repo2.token == repo1.token, "Token mismatch"
    assert repo2.subdomain == repo1.subdomain, f"Subdomain mismatch: {repo2.subdomain} != {repo1.subdomain}"


def test_get_dns_records():
    repo = get_repo()
    records = repo.dns()
    assert isinstance(records, list), f"Expected list, got {type(records)}"


def test_add_dns_record():
    repo = get_repo()
    result = repo.add_dns("test", "A", "1.2.3.4")
    assert result is True, "Failed to add DNS record"
    records = repo.dns()
    found = any(r.domain == "test" and r.type == "A" and r.value == "1.2.3.4" for r in records)
    assert found, "DNS record not found after adding"


def test_update_dns_records():
    repo = get_repo()
    new_records = [
        DnsRecord(type="A", domain="www", value="10.0.0.1"),
        DnsRecord(type="AAAA", domain="ipv6", value="::1"),
    ]
    result = repo.update_dns(new_records)
    assert result is True, "Failed to update DNS"
    records = repo.dns()
    assert len(records) == 2, f"Expected 2 records, got {len(records)}"


def test_remove_dns_record():
    repo = get_repo()
    repo.update_dns([])  # Clear first
    repo.add_dns("to-remove", "A", "1.1.1.1")
    repo.add_dns("to-keep", "A", "2.2.2.2")
    result = repo.remove_dns("to-remove")
    assert result is True, "Failed to remove DNS"
    records = repo.dns()
    assert not any(r.domain == "to-remove" for r in records), "Record not removed"


def test_get_files():
    repo = get_repo()
    files = repo.files()
    assert isinstance(files, dict), f"Expected dict, got {type(files)}"


def test_set_file():
    repo = get_repo()
    result = repo.set_file(
        path="test.txt",
        body="Hello, World!",
        status_code=200,
        headers=[Header(header="X-Custom", value="test")],
    )
    assert result is True, "Failed to set file"
    files = repo.files()
    assert "test.txt" in files, "File not found"
    assert files["test.txt"].status_code == 200, "Wrong status code"


def test_set_file_binary():
    repo = get_repo()
    content = b"\x00\x01\x02\x03"
    result = repo.set_file(path="binary.bin", body=content)
    assert result is True, "Failed to set binary file"
    files = repo.files()
    assert "binary.bin" in files, "Binary file not found"
    decoded = base64.b64decode(files["binary.bin"].raw)
    assert decoded == content, f"Content mismatch: {decoded} != {content}"


def test_list_requests():
    repo = get_repo()
    reqs = repo.list_requests()
    assert isinstance(reqs, list), f"Expected list, got {type(reqs)}"


def test_delete_all_requests():
    repo = get_repo()
    result = repo.delete_all_requests()
    assert result is True, "Failed to delete all requests"


def test_capture_http_request():
    repo = get_repo()
    repo.delete_all_requests()

    url = f"https://{repo.subdomain}.{repo.domain}/test-path?foo=bar"
    try:
        http_requests.get(url, timeout=5, headers={"X-Test": "integration"})
    except Exception:
        pass

    time.sleep(2)

    reqs = repo.list_requests()
    assert len(reqs) >= 1, f"No requests captured (got {len(reqs)})"
    http_reqs = [r for r in reqs if isinstance(r, HttpRequest)]
    assert len(http_reqs) >= 1, "No HTTP requests found"
    req = http_reqs[0]
    assert req.method == "GET", f"Wrong method: {req.method}"
    assert "/test-path" in req.path, f"Wrong path: {req.path}"


def test_delete_single_request():
    repo = get_repo()
    repo.delete_all_requests()

    url = f"https://{repo.subdomain}.{repo.domain}/to-delete"
    try:
        http_requests.get(url, timeout=5)
    except Exception:
        pass

    time.sleep(2)

    reqs = repo.list_requests()
    assert len(reqs) > 0, "No requests to delete"
    req_id = reqs[0].id
    result = repo.delete_request(req_id)
    assert result is True, "Failed to delete request"


def test_share_request():
    repo = get_repo()
    repo.delete_all_requests()

    url = f"https://{repo.subdomain}.{repo.domain}/shared-test"
    try:
        http_requests.get(url, timeout=5)
    except Exception:
        pass

    time.sleep(2)

    reqs = repo.list_requests()
    assert len(reqs) > 0, "No requests to share"

    req_id = reqs[0].id
    share_token = repo.share_request(req_id)
    assert share_token is not None, "No share token returned"
    assert len(share_token) > 0, "Empty share token"

    shared_req = repo.get_shared_request(share_token)
    assert shared_req is not None, "Failed to get shared request"
    assert shared_req.id == req_id, "Request ID mismatch"


def test_websocket_connection():
    repo = get_repo()
    repo.start_requests()

    try:
        time.sleep(1)
        result = repo.ping()
        assert result is True, "Ping failed"
    finally:
        repo.stop_requests()


def test_websocket_receive():
    received = []

    class TestRepo(Requestrepo):
        def on_request(self, req):
            received.append(req)

    # Use existing token to avoid rate limit
    base_repo = get_repo()
    repo = TestRepo(token=base_repo.token)
    repo.delete_all_requests()
    repo.start_requests()

    try:
        url = f"https://{repo.subdomain}.{repo.domain}/ws-test"
        try:
            http_requests.get(url, timeout=5)
        except Exception:
            pass

        time.sleep(3)

        assert len(received) >= 1, f"No requests received via WebSocket (got {len(received)})"
        assert isinstance(received[0], HttpRequest), f"Wrong type: {type(received[0])}"
    finally:
        repo.stop_requests()


def test_request_filters():
    http_req = HttpRequest(
        _id="1", type="http", raw=b"", uid="u", ip="1.1.1.1",
        date=0, method="GET", path="/", headers={}
    )
    dns_req = DnsRequest(
        _id="2", type="dns", raw=b"", uid="u", ip="1.1.1.1",
        date=0, query_type="A", domain="test.com"
    )

    assert Requestrepo.HTTP_FILTER(http_req) is True, "HTTP filter failed for HTTP"
    assert Requestrepo.HTTP_FILTER(dns_req) is False, "HTTP filter passed for DNS"
    assert Requestrepo.DNS_FILTER(http_req) is False, "DNS filter passed for HTTP"
    assert Requestrepo.DNS_FILTER(dns_req) is True, "DNS filter failed for DNS"


def test_pagination():
    repo = get_repo()
    repo.delete_all_requests()

    for i in range(3):
        url = f"https://{repo.subdomain}.{repo.domain}/page-{i}"
        try:
            http_requests.get(url, timeout=5)
        except Exception:
            pass

    time.sleep(3)

    all_reqs = repo.list_requests()
    limited = repo.list_requests(limit=2)

    assert len(limited) <= 2, f"Limit not working: got {len(limited)}"
    if len(all_reqs) >= 3:
        assert len(limited) == 2, f"Expected 2, got {len(limited)}"


def main():
    runner = TestRunner()

    print("=" * 60)
    print("Integration Tests - requestrepo.com")
    print("=" * 60)
    print()

    print("Session Tests:")
    runner.run_test("create_session", test_create_session)
    runner.run_test("create_session_with_token", test_create_session_with_token)

    print("\nDNS Tests:")
    runner.run_test("get_dns_records", test_get_dns_records)
    runner.run_test("add_dns_record", test_add_dns_record)
    runner.run_test("update_dns_records", test_update_dns_records)
    runner.run_test("remove_dns_record", test_remove_dns_record)

    print("\nFile Tests:")
    runner.run_test("get_files", test_get_files)
    runner.run_test("set_file", test_set_file)
    runner.run_test("set_file_binary", test_set_file_binary)

    print("\nRequest Tests:")
    runner.run_test("list_requests", test_list_requests)
    runner.run_test("delete_all_requests", test_delete_all_requests)
    runner.run_test("capture_http_request", test_capture_http_request)
    runner.run_test("delete_single_request", test_delete_single_request)

    print("\nSharing Tests:")
    runner.run_test("share_request", test_share_request)

    print("\nWebSocket Tests:")
    runner.run_test("websocket_connection", test_websocket_connection)
    runner.run_test("websocket_receive", test_websocket_receive)

    print("\nFilter Tests:")
    runner.run_test("request_filters", test_request_filters)

    print("\nPagination Tests:")
    runner.run_test("pagination", test_pagination)

    success = runner.summary()
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
