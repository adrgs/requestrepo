#!/usr/bin/env python3
import datetime
import os
import random
import threading

from dnslib import DNSRecord, QTYPE
from dnslib import A, AAAA, CNAME, TXT
from dnslib.server import DNSServer
from config import config
from utils import get_subdomain
from typing import Any
from models import DnsRequestLog, DnsEntry, Record
import json
import redis
import uuid
import base64
import ip2country


pool = redis.ConnectionPool(host=config.redis_host, port=6379, db=0)

class Resolver:
  def __init__(self, server_ip: str, server_domain: str) -> None:
    self.server_ip: str = server_ip
    self.server_domain: str = server_domain + "."

  def resolve_cname(self, reply: DNSRecord) -> Record | None:
    data = get_dns_record(str(reply.q.qname), "CNAME")
    if data is None:
      return Record(CNAME, self.server_domain)
    else:
      return Record(CNAME, data["value"])

  def resolve_txt(self, reply: DNSRecord) -> Record | None:
    data = get_dns_record(str(reply.q.qname), "TXT")
    if data is None:
      return Record(TXT, os.getenv("TXT") or "Hello!")
    else:
      return Record(TXT, data["value"])

  def resolve_ip(self, reply: DNSRecord, dtype: str) -> Record | None:
    new_record: Record | None = None
    data = get_dns_record(str(reply.q.qname), dtype)
    try:
      if data is None:
        new_record = Record(
          A if dtype == "A" else AAAA, self.server_ip)
      else:
        ips = data["value"]
        if "/" not in ips and "%" not in ips:
          new_record = Record(A, ips)
        else:
          if "%" in ips:
            ips_list = ips.split("%")
            idx = random.randint(0, len(ips_list) - 1)
            new_record = Record(A, ips_list[idx])
          else:
            ips_list = ips.split("/")
            new_record = Record(A, ips_list[0])
            ips = "/".join(ips_list[1:] + [ips_list[0]])
            update_dns_record(data["domain"], "A", ips)
    except:
      pass

    return new_record

  def resolve(self, request: DNSRecord, handler: Any) -> DNSRecord:
    reply: DNSRecord = request.reply()

    # We assume that the data in the DB is correct (using server side checks)
    new_record: Record | None = None

    if QTYPE[reply.q.qtype] == "CNAME":
      new_record = self.resolve_cname(reply)
    elif QTYPE[reply.q.qtype] == "TXT":
      new_record = self.resolve_txt(reply)
    elif QTYPE[reply.q.qtype] == "A":
      new_record = self.resolve_ip(reply, "A")
    elif QTYPE[reply.q.qtype] == "AAAA":
      new_record = self.resolve_ip(reply, "AAAA")
    else:
      return reply

    if not new_record is None:
      reply.add_answer(new_record.try_rr(request.q))

    save_into_db(
      reply,
      handler.client_address[0],
      handler.client_address[1],
      handler.request[0],
    )

    return reply


def save_into_db(reply: DNSRecord, ip: str, port: int, raw: bytes) -> None:
  name = str(reply.q.qname)
  uid = get_subdomain(name)

  if not uid:
    return

  dns_log = DnsRequestLog(
    type="dns",
    date=int(datetime.datetime.now(datetime.timezone.utc).timestamp()),
    ip=ip,
    port=port,
    dtype=str(QTYPE[reply.q.qtype]),
    name=name,
    uid=uid,
    reply=str(reply),
    raw=base64.b64encode(raw).decode(),
    _id=str(uuid.uuid4()),
  )

  country = ip2country.ip_to_country(ip)
  if not country is None:
    dns_log["country"] = country

  insert_into_db(dns_log)


def update_dns_record(domain: str, dtype: str, newval: str) -> None:
  r = redis.Redis(connection_pool=pool)

  dns_entry = DnsEntry(
    domain=domain,
    type=dtype,
    value=newval,
    _id=str(uuid.uuid4()),
  )

  result = r.get(f"dns:{dtype}:{domain}")

  if result:
    data = json.loads(result)
    dns_entry["_id"] = data["_id"]

  r.set(f"dns:{dtype}:{domain}", json.dumps(dns_entry))


def insert_into_db(value: DnsRequestLog) -> None:
  r = redis.Redis(connection_pool=pool)

  subdomain = value["uid"]
  data = json.dumps(value)

  r.publish(f"pubsub:{subdomain}", data)
  idx = r.rpush(f"requests:{subdomain}", data) - 1
  r.set(f"request:{subdomain}:{value['_id']}", idx)


def get_dns_record(domain: str, dtype: str) -> DnsEntry | None:
  r = redis.Redis(connection_pool=pool)

  domain = domain.lower()

  result = r.get(f"dns:{dtype}:{domain}")

  if result:
    return json.loads(result)

  return None


resolver = Resolver(config.server_ip, config.server_domain)
servers = [
  DNSServer(resolver, port=53, address="0.0.0.0", tcp=True),
  DNSServer(resolver, port=53, address="0.0.0.0", tcp=False),
]

if __name__ == "__main__":
  print("Starting DNS server...")
  stop_event = threading.Event()

  for s in servers:
    s.start_thread()

  try:
    stop_event.wait()
  except KeyboardInterrupt:
    print("Stopping DNS server...")
  finally:
    for s in servers:
      s.stop()
