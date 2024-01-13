import functools
import bisect
import gzip
import os

show_country = False
ip_list: list[tuple[int, str]] = []


def get_directory() -> str:
  return os.path.dirname(os.path.realpath(__file__))


@functools.lru_cache(maxsize=1024)
def ip_to_country(ip: str) -> str | None:
  if not is_ipv4(ip) or not show_country:
    return None

  ip_int = ipv4_to_int(ip)
  idx = bisect.bisect_right(ip_list, (ip_int, 'ZZ'))
  if idx == 0:
    return None
  return ip_list[idx - 1][1]


def ipv4_to_int(ip: str) -> int:
  octets = ip.split('.')
  return (int(octets[0]) << 24) + (int(octets[1]) << 16) + (int(octets[2]) << 8) + int(octets[3])


def is_ipv4(s: str) -> bool:
  try:
    return all(0 <= int(p) < 256 for p in s.split("."))
  except ValueError:
    return False


if os.path.exists(get_directory()+'/vendor/dbip-country-lite.csv.gz'):
  show_country = True
  with gzip.open(get_directory()+'/vendor/dbip-country-lite.csv.gz', 'rb') as f:
    for line in f:
      ip_from, ip_to, country = line.decode('utf-8').split(',')
      if not is_ipv4(ip_from):
        continue
      ip_list.append((ipv4_to_int(ip_from), country.strip()))
