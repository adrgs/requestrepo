import random
import json
import jwt
from config import config
from typing import TypedDict, Union
from redis import asyncio as aioredis


def verify_subdomain(
    subdomain: str,
    length: int = config.subdomain_length,
    alphabet_set: set = config.subdomain_alphabet_set,
) -> bool:
    return (
        isinstance(subdomain, str)
        and len(subdomain) == length
        and set(subdomain).issubset(alphabet_set)
    )


def verify_jwt(
    token: str,
    length: int = config.subdomain_length,
    alphabet_set: set = config.subdomain_alphabet_set,
) -> Union[str, None]:
    try:
        decoded_token = jwt.decode(token, config.jwt_secret, algorithms=["HS256"])
        subdomain = decoded_token.get("subdomain")
        if verify_subdomain(subdomain, length, alphabet_set):
            return subdomain
    except Exception:
        pass

    return None


def get_random_subdomain(
    alphabet: str = config.subdomain_alphabet, length: int = config.subdomain_length
) -> str:
    return "".join(random.choices(alphabet, k=length))


def get_subdomain_from_path(
    path: str,
    length: int = config.subdomain_length,
    alphabet_set: set = config.subdomain_alphabet_set,
):
    if not path:
        return None

    path = path.lower()

    path = path.lstrip("/")
    if not path.startswith("r/"):
        return None
    path = path[2:].lstrip("/")

    subdomain = path[:length]

    if len(subdomain) != length or set(subdomain) - alphabet_set != set():
        return None

    return subdomain


def get_subdomain_from_hostname(
    host: str,
    domain: str = config.server_domain,
    length: int = config.subdomain_length,
    alphabet_set: set = config.subdomain_alphabet_set,
):
    if not host:
        return None

    host = host.lower()

    r_index = host.rfind(domain)
    if r_index < length + 1:
        return None

    subdomain = host[r_index - 1 - length : r_index - 1]

    if (
        not subdomain
        or len(subdomain) != length
        or set(subdomain) - alphabet_set != set()
    ):
        return None

    return subdomain


class RequestRepoHeader(TypedDict):
    header: str
    value: str


class RequestRepoResponse(TypedDict):
    raw: str
    headers: list[RequestRepoHeader]
    status_code: int


async def write_basic_file(subdomain: str, redis: aioredis.Redis) -> None:
    file_data = {
        "headers": [
            {"header": "Access-Control-Allow-Origin", "value": "*"},
            {"header": "Content-Type", "value": "text/html; charset=utf-8"},
        ],
        "status_code": 200,
        "raw": "",
    }

    if config.include_server_domain:
        file_data["headers"].append({"header": "Server", "value": config.server_domain})

    tree = {"index.html": file_data}

    await redis.set(f"files:{subdomain}", json.dumps(tree), ex=config.redis_ttl)
