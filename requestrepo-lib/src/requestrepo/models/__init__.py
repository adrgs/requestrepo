"""Data models for requestrepo v2 API.

This module provides Pydantic models for all request types, DNS records,
and HTTP response configurations used by the requestrepo API.
"""

from typing import Optional

from pydantic import BaseModel, Field


class HttpRequest(BaseModel):
    """HTTP request captured by requestrepo.

    Attributes:
        id: Unique identifier for the request.
        type: Request type, always "http".
        raw: Raw request data as bytes.
        uid: Session identifier.
        ip: Source IP address.
        country: Two-letter country code from IP geolocation.
        date: Unix timestamp of when the request was received.
        method: HTTP method (GET, POST, etc.).
        path: Request path including query string.
        http_version: HTTP version (e.g., "HTTP/1.1").
        headers: Dictionary of HTTP headers.
        body: Request body as bytes, if present.
    """

    id: str = Field(..., alias="_id")
    type: str
    raw: bytes
    uid: str
    ip: str
    country: Optional[str] = None
    date: int
    method: str
    path: str
    http_version: Optional[str] = None
    headers: dict[str, str]
    body: Optional[bytes] = None

    model_config = {"populate_by_name": True}


class DnsRequest(BaseModel):
    """DNS request captured by requestrepo.

    Attributes:
        id: Unique identifier for the request.
        type: Request type, always "dns".
        raw: Raw DNS query data as bytes.
        uid: Session identifier.
        ip: Source IP address.
        country: Two-letter country code from IP geolocation.
        port: Source port number.
        date: Unix timestamp of when the request was received.
        query_type: DNS query type (A, AAAA, CNAME, TXT, etc.).
        domain: Queried domain name.
        reply: DNS reply sent back, if any.
    """

    id: str = Field(..., alias="_id")
    type: str
    raw: bytes
    uid: str
    ip: str
    country: Optional[str] = None
    port: Optional[int] = None
    date: int
    query_type: str
    domain: str
    reply: Optional[str] = None

    model_config = {"populate_by_name": True}


class SmtpRequest(BaseModel):
    """SMTP request captured by requestrepo.

    Attributes:
        id: Unique identifier for the request.
        type: Request type, always "smtp".
        raw: Raw SMTP data as bytes.
        uid: Session identifier.
        ip: Source IP address.
        country: Two-letter country code from IP geolocation.
        date: Unix timestamp of when the request was received.
        command: SMTP command received.
        data: Email body data.
        subject: Email subject line.
        from_addr: Sender email address.
        to: Recipient email address.
        cc: CC recipients.
        bcc: BCC recipients.
    """

    id: str = Field(..., alias="_id")
    type: str
    raw: bytes
    uid: str
    ip: str
    country: Optional[str] = None
    date: int
    command: str
    data: Optional[str] = None
    subject: Optional[str] = None
    from_addr: Optional[str] = Field(None, alias="from")
    to: Optional[str] = None
    cc: Optional[str] = None
    bcc: Optional[str] = None

    model_config = {"populate_by_name": True}


class TcpRequest(BaseModel):
    """TCP request captured by requestrepo.

    Attributes:
        id: Unique identifier for the request.
        type: Request type, always "tcp".
        raw: Raw TCP data as bytes.
        uid: Session identifier.
        ip: Source IP address.
        country: Two-letter country code from IP geolocation.
        port: TCP port number.
        date: Unix timestamp of when the request was received.
    """

    id: str = Field(..., alias="_id")
    type: str
    raw: bytes
    uid: str
    ip: str
    country: Optional[str] = None
    port: int
    date: int

    model_config = {"populate_by_name": True}


class DnsRecord(BaseModel):
    """DNS record configuration for a session.

    Attributes:
        type: DNS record type (A, AAAA, CNAME, TXT).
        domain: Domain or subdomain name.
        value: Record value (IP address, hostname, or text).
    """

    type: str
    domain: str
    value: str


class Header(BaseModel):
    """HTTP header for custom responses.

    Attributes:
        header: Header name.
        value: Header value.
    """

    header: str
    value: str


class Response(BaseModel):
    """Custom HTTP response configuration for the file tree.

    Attributes:
        raw: Base64 encoded response body.
        headers: List of HTTP headers to include.
        status_code: HTTP status code to return.
    """

    raw: str
    headers: list[Header]
    status_code: int


__all__ = [
    "HttpRequest",
    "DnsRequest",
    "SmtpRequest",
    "TcpRequest",
    "DnsRecord",
    "Header",
    "Response",
]
