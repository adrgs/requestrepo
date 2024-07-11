import sys
from typing import TypedDict, List
from pydantic import BaseModel

if sys.version_info < (3, 11):
    from typing_extensions import NotRequired
else:
    from typing import NotRequired


class HttpRequestLog(TypedDict):
    _id: str
    type: str
    raw: str
    uid: str
    ip: str
    country: NotRequired[str]
    port: int
    headers: dict[str, str]
    method: str
    protocol: str
    path: str
    fragment: str
    query: str
    url: str
    date: int


class Header(BaseModel):
    header: str
    value: str


class File(BaseModel):
    raw: str
    headers: List[Header]
    status_code: int


class DeleteRequest(BaseModel):
    id: str


class DnsRecord(BaseModel):
    domain: str
    type: int
    value: str


class DnsRecords(BaseModel):
    records: List[DnsRecord]


class RequestRepoHeader(TypedDict):
    header: str
    value: str


class RequestRepoResponse(TypedDict):
    raw: str
    headers: list[RequestRepoHeader]
    status_code: int
