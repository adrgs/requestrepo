#!/usr/bin/env python3
import datetime
import os
import random
import threading

from dnslib import DNSRecord, QTYPE, RD, RR
from dnslib import A, AAAA, CNAME, MX, NS, SOA, TXT
from dnslib.server import DNSServer
from config import config
from utils import get_subdomain
from typing import Any
import json
import redis
import uuid
import base64


EPOCH: datetime.datetime = datetime.datetime(1970, 1, 1)
SERIAL: int = int(datetime.datetime.now(datetime.timezone.utc).timestamp())

TYPE_LOOKUP: dict[A | AAAA | CNAME | MX | NS | SOA | TXT, int] = {
    A: QTYPE.A,
    AAAA: QTYPE.AAAA,
    CNAME: QTYPE.CNAME,
    MX: QTYPE.MX,
    NS: QTYPE.NS,
    SOA: QTYPE.SOA,
    TXT: QTYPE.TXT,
}


class Record:
    def __init__(
        self,
        rdata_type: RD | A | AAAA | CNAME | MX | NS | SOA | TXT,
        *args,
        rtype=None,
        rname=None,
        ttl=None,
        **kwargs,
    ) -> None:
        if isinstance(rdata_type, RD):
            self._rtype = TYPE_LOOKUP[rdata_type.__class__]
            rdata = rdata_type
        else:
            self._rtype = TYPE_LOOKUP[rdata_type]
            if rdata_type == SOA and len(args) == 2:
                args += (
                    (
                        SERIAL,  # serial number
                        60 * 60 * 1,  # refresh
                        60 * 60 * 3,  # retry
                        60 * 60 * 24,  # expire
                        60 * 60 * 1,  # minimum
                    ),
                )
            rdata = rdata_type(*args)

        if rtype:
            self._rtype = rtype
        self._rname = rname
        self.kwargs = dict(
            rdata=rdata, ttl=self.sensible_ttl() if ttl is None else ttl, **kwargs
        )

    def try_rr(self, q) -> RR | None:
        if q.qtype == QTYPE.ANY or q.qtype == self._rtype:
            return self.as_rr(q.qname)

    def as_rr(self, alt_rname) -> RR:
        return RR(rname=self._rname or alt_rname, rtype=self._rtype, **self.kwargs)

    def sensible_ttl(self) -> int:
        return 1

    @property
    def is_soa(self) -> bool:
        return self._rtype == QTYPE.SOA

    def __str__(self) -> str:
        return "{} {}".format(QTYPE[self._rtype], self.kwargs)


def update_dns_record(domain: str, dtype: str, newval: str) -> None:
    r = redis.Redis(host=config.redis_host, port=6379, db=0)

    data = {"domain": domain, "type": dtype,
            "value": newval, "_id": str(uuid.uuid4())}

    result = r.get(f"dns:{dtype}:{domain}")

    if result:
        result = json.loads(result)
        data["_id"] = result["_id"]

    r.set(f"dns:{dtype}:{domain}", json.dumps(data))


def save_into_db(reply: DNSRecord, ip: str, port: int, raw: bytes) -> None:
    name = str(reply.q.qname)
    uid = get_subdomain(name)

    if not uid:
        return

    data = {
        "type": "dns",
        "date": int(datetime.datetime.now(datetime.timezone.utc).timestamp()),
        "ip": ip,
        "port": port,
        "dtype": QTYPE[reply.q.qtype],
        "name": name,
        "uid": uid,
        "reply": str(reply),
        "raw": base64.b64encode(raw).decode(),
        "_id": str(uuid.uuid4()),
    }
    insert_into_db(data)


def insert_into_db(value: dict[str, Any]) -> None:
    r = redis.Redis(host=config.redis_host, port=6379, db=0)

    subdomain = value["uid"]
    data = json.dumps(value)

    r.publish(f"pubsub:{subdomain}", data)
    idx = r.rpush(f"requests:{subdomain}", data) - 1
    r.set(f"request:{subdomain}:{value['_id']}", idx)


def get_dns_record(domain: str, dtype: str) -> dict[str, Any] | None:
    r = redis.Redis(host=config.redis_host, port=6379, db=0)

    domain = domain.lower()

    result = r.get(f"dns:{dtype}:{domain}")

    if result:
        return json.loads(result)


class Resolver:
    def __init__(self, server_ip: str, server_domain: str) -> None:
        self.server_ip: str = server_ip
        self.server_domain: str = server_domain + "."

    def resolve_cname(self, reply: DNSRecord) -> Record | None:
        data = get_dns_record(str(reply.q.qname), "CNAME")
        if data == None:
            return Record(CNAME, self.server_domain)
        else:
            return Record(CNAME, data["value"])

    def resolve_txt(self, reply: DNSRecord) -> Record | None:
        data = get_dns_record(str(reply.q.qname), "TXT")
        if data == None:
            return Record(TXT, os.getenv("TXT") or "Hello!")
        else:
            return Record(TXT, data["value"])

    def resolve_ip(self, reply: DNSRecord, dtype: str) -> Record | None:
        new_record: None | Record = None
        data = get_dns_record(str(reply.q.qname), dtype)
        if data == None:
            new_record = Record(A if dtype == "A" else AAAA, self.server_ip)
        else:
            ips = data["value"]
            if "/" not in ips and "%" not in ips:
                new_record = Record(A, ips)
            else:
                if "%" in ips:
                    ips = ips.split("%")
                    idx = random.randint(0, len(ips) - 1)
                    if "/" in ips[idx]:
                        new_ips = ips[idx].split("/")
                        new_record = Record(A, new_ips[0])
                        new_ips = "/".join(new_ips[1:] + [new_ips[0]])
                        ips[idx] = new_ips
                        ips = "%".join(ips)
                        update_dns_record(data["domain"], "A", ips)
                    else:
                        new_record = Record(A, ips[idx])
                else:
                    ips = ips.split("/")
                    new_record = Record(A, ips[0])
                    ips = "/".join(ips[1:] + [ips[0]])
                    update_dns_record(data["domain"], "A", ips)

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

        reply.add_answer(new_record.try_rr(request.q))
        save_into_db(
            reply,
            handler.client_address[0],
            handler.client_address[1],
            handler.request[0],
        )

        return reply


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
