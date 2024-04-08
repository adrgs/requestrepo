import random
import json
import jwt
from pathlib import Path
from config import config
from typing import TypedDict


def verify_jwt(token: str) -> str | None:
  try:
    dic = jwt.decode(token, config.jwt_secret, algorithms=["HS256"])
    if "subdomain" in dic and type(dic["subdomain"]) == str:
      return dic["subdomain"]
    return None
  except Exception:
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

  while path.startswith("/"):
    path = path[1:]

  subdomain = path[:length]

  if len(subdomain) != length or set(subdomain) - alphabet_set != set() or subdomain in config.reserved_keywords:
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
  subdomain = host[r_index - 1 - length: r_index - 1]

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


def write_basic_file(subdomain: str):
  file_data = RequestRepoResponse(
    headers=[
      {"header": "Access-Control-Allow-Origin", "value": "*"},
      {"header": "Content-Type", "value": "text/html"},
    ],
    status_code=200,
    raw="",
  )

  if config.include_server_domain:
    file_data["headers"].append(
      {"header": "Server", "value": config.server_domain})

  with open(Path("pages/") / Path(subdomain).name, "w") as outfile:
    json.dump(file_data, outfile)
