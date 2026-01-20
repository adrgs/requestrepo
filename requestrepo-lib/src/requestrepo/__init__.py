"""Requestrepo Python client library for the v2 API.

This module provides a Python client for interacting with requestrepo,
an HTTP/DNS/SMTP request analysis tool. It supports real-time request
streaming via WebSocket and REST API operations.

Example:
    Basic usage::

        from requestrepo import Requestrepo

        repo = Requestrepo()
        print(f"Your subdomain: {repo.subdomain}.{repo.domain}")

    Real-time streaming::

        class MyRepo(Requestrepo):
            def on_request(self, req):
                print(f"Received {req.type} request from {req.ip}")

        repo = MyRepo()
        repo.await_requests()
"""

import asyncio
import base64
import json
import threading
from typing import Callable, Union

import requests
from websockets.sync.client import connect

from .models import (
    DnsRecord,
    DnsRequest,
    Header,
    HttpRequest,
    Response,
    SmtpRequest,
    TcpRequest,
)

# Type alias for any request type
AnyRequest = Union[HttpRequest, DnsRequest, SmtpRequest, TcpRequest]


class Requestrepo:
    """Client for the requestrepo v2 API.

    This class provides methods to interact with requestrepo for capturing
    and analyzing HTTP, DNS, SMTP, and TCP requests. It supports both
    synchronous REST API calls and real-time WebSocket streaming.

    Attributes:
        subdomain: The unique subdomain assigned to this session.
        domain: The base domain of the requestrepo instance.

    Args:
        token: Existing JWT token to use. If not provided, a new session is created.
        admin_token: Admin token for creating sessions on protected instances.
        host: Hostname of the requestrepo instance.
        port: Port number of the requestrepo instance.
        protocol: Protocol to use ('http' or 'https').
        verify: Whether to verify SSL certificates.

    Raises:
        requests.HTTPError: If session creation fails.
    """

    def __init__(
        self,
        token: str | None = None,
        admin_token: str | None = None,
        host: str = "requestrepo.com",
        port: int = 443,
        protocol: str = "https",
        verify: bool = True,
    ) -> None:
        """Initialize the requestrepo client.

        Creates a new session if no token is provided. The session provides
        a unique subdomain for capturing requests.
        """
        self.__host = host
        self.__port = port
        self.__protocol = protocol
        self.__verify = verify
        self.__websocket = None
        self.__ws_thread = None
        self.__ws_running = False
        self.__request_queue: list[AnyRequest] = []
        self.__request_event = threading.Event()

        if not token:
            body: dict = {}
            if admin_token:
                body["admin_token"] = admin_token
            resp = requests.post(
                f"{protocol}://{host}:{port}/api/v2/sessions",
                json=body,
                verify=verify,
            )
            resp.raise_for_status()
            data = resp.json()
            token = data["token"]
            self.__subdomain = data["subdomain"]
        else:
            # Extract subdomain from token by decoding JWT payload
            self.__subdomain = self._extract_subdomain_from_token(token)

        self.__token = token

    def _extract_subdomain_from_token(self, token: str) -> str:
        """Extract the subdomain from a JWT token.

        Args:
            token: The JWT token to decode.

        Returns:
            The subdomain extracted from the token payload.
        """
        try:
            # JWT is base64url encoded: header.payload.signature
            payload = token.split(".")[1]
            # Add padding if needed
            padding = 4 - len(payload) % 4
            if padding != 4:
                payload += "=" * padding
            decoded = base64.urlsafe_b64decode(payload)
            data = json.loads(decoded)
            return data.get("subdomain", "")
        except (IndexError, ValueError, json.JSONDecodeError):
            return ""

    def _auth_headers(self) -> dict[str, str]:
        """Get authorization headers for API requests.

        Returns:
            Dictionary with Bearer token authorization header.
        """
        return {"Authorization": f"Bearer {self.__token}"}

    def _base_url(self) -> str:
        """Get the base URL for API requests.

        Returns:
            The base URL including protocol, host, and port.
        """
        return f"{self.__protocol}://{self.__host}:{self.__port}"

    @property
    def subdomain(self) -> str:
        """Get the subdomain assigned to this session.

        Returns:
            The unique subdomain string.
        """
        return self.__subdomain

    @property
    def domain(self) -> str:
        """Get the base domain of the requestrepo instance.

        Returns:
            The base domain string.
        """
        return self.__host

    @property
    def token(self) -> str:
        """Get the JWT token for this session.

        Returns:
            The JWT token string.
        """
        return self.__token

    # -------------------------------------------------------------------------
    # Request Parsing
    # -------------------------------------------------------------------------

    def _parse_request(self, data: dict) -> AnyRequest:
        """Parse a request dictionary into the appropriate model.

        Args:
            data: Dictionary containing request data from the API.

        Returns:
            The parsed request object (HttpRequest, DnsRequest, SmtpRequest, or TcpRequest).

        Raises:
            ValueError: If the request type is unknown.
        """
        req_type = data.get("type")

        # Decode base64 raw data if present
        if "raw" in data and isinstance(data["raw"], str):
            data["raw"] = base64.b64decode(data["raw"])

        # Decode base64 body for HTTP requests
        if "body" in data and isinstance(data["body"], str):
            data["body"] = base64.b64decode(data["body"])

        if req_type == "http":
            return HttpRequest(**data)
        elif req_type == "dns":
            return DnsRequest(**data)
        elif req_type == "smtp":
            return SmtpRequest(**data)
        elif req_type == "tcp":
            return TcpRequest(**data)
        else:
            raise ValueError(f"Unknown request type: {req_type}")

    # -------------------------------------------------------------------------
    # DNS Operations
    # -------------------------------------------------------------------------

    def dns(self) -> list[DnsRecord]:
        """Get all DNS records for this session.

        Returns:
            List of DNS records configured for this session.

        Raises:
            requests.HTTPError: If the request fails.
        """
        r = requests.get(
            f"{self._base_url()}/api/v2/dns",
            headers=self._auth_headers(),
            verify=self.__verify,
        )
        r.raise_for_status()
        return [DnsRecord(**d) for d in r.json().get("records", [])]

    def update_dns(self, dns_records: list[DnsRecord]) -> bool:
        """Update all DNS records for this session.

        Args:
            dns_records: List of DNS records to set.

        Returns:
            True if the update was successful, False otherwise.
        """
        r = requests.put(
            f"{self._base_url()}/api/v2/dns",
            headers=self._auth_headers(),
            json={"records": [d.model_dump() for d in dns_records]},
            verify=self.__verify,
        )
        return r.status_code == 200

    def add_dns(self, domain: str, record_type: str, value: str) -> bool:
        """Add a DNS record to this session.

        Args:
            domain: Domain or subdomain name.
            record_type: DNS record type (A, AAAA, CNAME, TXT).
            value: Record value.

        Returns:
            True if the record was added successfully, False otherwise.
        """
        records = self.dns()
        records.append(DnsRecord(type=record_type, domain=domain, value=value))
        return self.update_dns(records)

    def remove_dns(self, domain: str, record_type: str | None = None) -> bool:
        """Remove DNS records matching the given criteria.

        Args:
            domain: Domain name to match.
            record_type: Optional record type to match. If not provided,
                        all records for the domain are removed.

        Returns:
            True if any records were removed successfully, False otherwise.
        """
        records = self.dns()
        if record_type:
            filtered = [
                r for r in records if not (r.domain == domain and r.type == record_type)
            ]
        else:
            filtered = [r for r in records if r.domain != domain]
        return self.update_dns(filtered)

    # -------------------------------------------------------------------------
    # Files Operations (FileTree)
    # -------------------------------------------------------------------------

    def files(self) -> dict[str, Response]:
        """Get all response files for this session.

        Returns:
            Dictionary mapping file paths to Response objects.

        Raises:
            requests.HTTPError: If the request fails.
        """
        r = requests.get(
            f"{self._base_url()}/api/v2/files",
            headers=self._auth_headers(),
            verify=self.__verify,
        )
        r.raise_for_status()
        return {k: Response(**v) for k, v in r.json().items()}

    def update_files(self, files: dict[str, Response]) -> bool:
        """Update all response files for this session.

        Args:
            files: Dictionary mapping file paths to Response objects.

        Returns:
            True if the update was successful, False otherwise.
        """
        r = requests.put(
            f"{self._base_url()}/api/v2/files",
            headers=self._auth_headers(),
            json={k: v.model_dump() for k, v in files.items()},
            verify=self.__verify,
        )
        return r.status_code == 200

    def get_file(self, path: str) -> Response:
        """Get a single response file by path.

        Args:
            path: The file path to retrieve.

        Returns:
            The Response object for the given path.

        Raises:
            requests.HTTPError: If the file is not found or request fails.
        """
        r = requests.get(
            f"{self._base_url()}/api/v2/files/{path}",
            headers=self._auth_headers(),
            verify=self.__verify,
        )
        r.raise_for_status()
        return Response(**r.json())

    def set_file(
        self,
        path: str,
        body: str | bytes,
        status_code: int = 200,
        headers: list[Header] | None = None,
    ) -> bool:
        """Set or update a single response file.

        Args:
            path: The file path to set.
            body: The response body (will be base64 encoded if bytes).
            status_code: HTTP status code to return.
            headers: List of headers to include in the response.

        Returns:
            True if the file was set successfully, False otherwise.
        """
        all_files = self.files()

        if isinstance(body, bytes):
            raw = base64.b64encode(body).decode("utf-8")
        else:
            raw = base64.b64encode(body.encode("utf-8")).decode("utf-8")

        all_files[path] = Response(
            raw=raw,
            headers=headers or [],
            status_code=status_code,
        )
        return self.update_files(all_files)

    # -------------------------------------------------------------------------
    # Request Operations
    # -------------------------------------------------------------------------

    def list_requests(
        self, limit: int = 100, offset: int = 0
    ) -> list[AnyRequest]:
        """List captured requests with pagination.

        Args:
            limit: Maximum number of requests to return.
            offset: Number of requests to skip.

        Returns:
            List of request objects.

        Raises:
            requests.HTTPError: If the request fails.
        """
        r = requests.get(
            f"{self._base_url()}/api/v2/requests",
            params={"limit": limit, "offset": offset},
            headers=self._auth_headers(),
            verify=self.__verify,
        )
        r.raise_for_status()
        return [self._parse_request(req) for req in r.json().get("requests", [])]

    def get_request(
        self, filter_func: Callable[[AnyRequest], bool] | None = None
    ) -> AnyRequest:
        """Get a single request, optionally filtered.

        Blocks until a request matching the filter is available. If no filter
        is provided, returns the next available request.

        Args:
            filter_func: Optional function to filter requests.

        Returns:
            The first request matching the filter.
        """
        # First check the queue for matching requests
        for i, req in enumerate(self.__request_queue):
            if filter_func is None or filter_func(req):
                return self.__request_queue.pop(i)

        # Wait for new requests
        while True:
            self.__request_event.wait()
            self.__request_event.clear()

            for i, req in enumerate(self.__request_queue):
                if filter_func is None or filter_func(req):
                    return self.__request_queue.pop(i)

    def delete_request(self, request_id: str) -> bool:
        """Delete a single request by ID.

        Args:
            request_id: The ID of the request to delete.

        Returns:
            True if the request was deleted successfully, False otherwise.
        """
        r = requests.delete(
            f"{self._base_url()}/api/v2/requests/{request_id}",
            headers=self._auth_headers(),
            verify=self.__verify,
        )
        return r.status_code == 200

    def delete_all_requests(self) -> bool:
        """Delete all captured requests for this session.

        Returns:
            True if all requests were deleted successfully, False otherwise.
        """
        r = requests.delete(
            f"{self._base_url()}/api/v2/requests",
            headers=self._auth_headers(),
            verify=self.__verify,
        )
        return r.status_code == 200

    # -------------------------------------------------------------------------
    # Request Sharing
    # -------------------------------------------------------------------------

    def share_request(self, request_id: str) -> str:
        """Create a share token for a request.

        Args:
            request_id: The ID of the request to share.

        Returns:
            A share token that can be used to access the request without authentication.

        Raises:
            requests.HTTPError: If the request fails.
        """
        r = requests.post(
            f"{self._base_url()}/api/v2/requests/{request_id}/share",
            headers=self._auth_headers(),
            verify=self.__verify,
        )
        r.raise_for_status()
        return r.json()["share_token"]

    def get_shared_request(self, share_token: str) -> AnyRequest:
        """Get a request by its share token.

        This method does not require authentication.

        Args:
            share_token: The share token for the request.

        Returns:
            The shared request object.

        Raises:
            requests.HTTPError: If the token is invalid or expired.
        """
        r = requests.get(
            f"{self._base_url()}/api/v2/requests/shared/{share_token}",
            verify=self.__verify,
        )
        r.raise_for_status()
        return self._parse_request(r.json())

    # -------------------------------------------------------------------------
    # Request Filters
    # -------------------------------------------------------------------------

    @staticmethod
    def HTTP_FILTER(request: AnyRequest) -> bool:
        """Filter for HTTP requests.

        Args:
            request: The request to check.

        Returns:
            True if the request is an HTTP request.
        """
        return isinstance(request, HttpRequest)

    @staticmethod
    def DNS_FILTER(request: AnyRequest) -> bool:
        """Filter for DNS requests.

        Args:
            request: The request to check.

        Returns:
            True if the request is a DNS request.
        """
        return isinstance(request, DnsRequest)

    @staticmethod
    def SMTP_FILTER(request: AnyRequest) -> bool:
        """Filter for SMTP requests.

        Args:
            request: The request to check.

        Returns:
            True if the request is an SMTP request.
        """
        return isinstance(request, SmtpRequest)

    @staticmethod
    def TCP_FILTER(request: AnyRequest) -> bool:
        """Filter for TCP requests.

        Args:
            request: The request to check.

        Returns:
            True if the request is a TCP request.
        """
        return isinstance(request, TcpRequest)

    def get_http_request(self) -> HttpRequest:
        """Get the next HTTP request.

        Blocks until an HTTP request is available.

        Returns:
            The next HTTP request.
        """
        return self.get_request(Requestrepo.HTTP_FILTER)  # type: ignore

    def get_dns_request(self) -> DnsRequest:
        """Get the next DNS request.

        Blocks until a DNS request is available.

        Returns:
            The next DNS request.
        """
        return self.get_request(Requestrepo.DNS_FILTER)  # type: ignore

    def get_smtp_request(self) -> SmtpRequest:
        """Get the next SMTP request.

        Blocks until an SMTP request is available.

        Returns:
            The next SMTP request.
        """
        return self.get_request(Requestrepo.SMTP_FILTER)  # type: ignore

    def get_tcp_request(self) -> TcpRequest:
        """Get the next TCP request.

        Blocks until a TCP request is available.

        Returns:
            The next TCP request.
        """
        return self.get_request(Requestrepo.TCP_FILTER)  # type: ignore

    # -------------------------------------------------------------------------
    # WebSocket Operations
    # -------------------------------------------------------------------------

    def _connect_websocket(self) -> None:
        """Connect to the WebSocket.

        Raises:
            Exception: If the WebSocket connection fails.
        """
        ws_protocol = "wss" if self.__protocol == "https" else "ws"
        self.__websocket = connect(
            f"{ws_protocol}://{self.__host}:{self.__port}/api/v2/ws"
        )

        # Send connect command
        self.__websocket.send(json.dumps({"cmd": "connect", "token": self.__token}))

        # Wait for connected response
        msg = json.loads(self.__websocket.recv())
        if msg.get("cmd") == "error":
            raise Exception(f"WebSocket error: {msg.get('message')}")

    def _ws_loop(self) -> None:
        """Internal WebSocket message loop."""
        while self.__ws_running and self.__websocket:
            try:
                raw_msg = self.__websocket.recv()
                msg = json.loads(raw_msg)
                cmd = msg.get("cmd")

                if cmd == "request":
                    request = self._parse_request(msg.get("data", {}))
                    self.__request_queue.append(request)
                    self.__request_event.set()
                    self.on_request(request)
                elif cmd == "requests":
                    # Handle batch of historical requests
                    for req_data in msg.get("data", []):
                        request = self._parse_request(req_data)
                        self.__request_queue.append(request)
                        self.__request_event.set()
                        self.on_request(request)
                elif cmd == "pong":
                    pass
                elif cmd == "deleted":
                    self.on_deleted(msg.get("data", {}).get("id"))
                elif cmd == "cleared":
                    self.on_cleared()

            except Exception:
                if self.__ws_running:
                    self.__ws_running = False
                break

    def on_request(self, request: AnyRequest) -> None:
        """Called when a new request is received.

        Override this method in a subclass to handle incoming requests.

        Args:
            request: The received request object.
        """
        pass

    def on_deleted(self, request_id: str | None) -> None:
        """Called when a request is deleted.

        Override this method in a subclass to handle deletion events.

        Args:
            request_id: The ID of the deleted request.
        """
        pass

    def on_cleared(self) -> None:
        """Called when all requests are cleared.

        Override this method in a subclass to handle clear events.
        """
        pass

    def await_requests(self) -> None:
        """Start listening for real-time requests via WebSocket.

        This method connects to the WebSocket and blocks until the
        connection is closed. Override `on_request` to handle incoming
        requests.

        Example:
            class MyRepo(Requestrepo):
                def on_request(self, req):
                    print(f"Got {req.type} request")

            repo = MyRepo()
            repo.await_requests()
        """
        self._connect_websocket()
        self.__ws_running = True
        self._ws_loop()

    def start_requests(self) -> None:
        """Start listening for requests in a background thread.

        This method is non-blocking. Use `stop_requests` to stop listening.

        Example:
            repo = Requestrepo()
            repo.start_requests()
            # Do other work...
            request = repo.get_http_request()
            repo.stop_requests()
        """
        self._connect_websocket()
        self.__ws_running = True
        self.__ws_thread = threading.Thread(target=self._ws_loop, daemon=True)
        self.__ws_thread.start()

    def stop_requests(self) -> None:
        """Stop listening for requests.

        Closes the WebSocket connection and stops the background thread
        if running.
        """
        self.__ws_running = False
        if self.__websocket:
            try:
                self.__websocket.close()
            except Exception:
                pass
            self.__websocket = None
        if self.__ws_thread:
            self.__ws_thread.join(timeout=1.0)
            self.__ws_thread = None

    def ping(self) -> bool:
        """Send a ping to the WebSocket server.

        Returns:
            True if the pong was received, False otherwise.
        """
        if not self.__websocket:
            return False
        try:
            self.__websocket.send(json.dumps({"cmd": "ping"}))
            return True
        except Exception:
            return False


__all__ = [
    "Requestrepo",
    "HttpRequest",
    "DnsRequest",
    "SmtpRequest",
    "TcpRequest",
    "DnsRecord",
    "Header",
    "Response",
    "AnyRequest",
]
