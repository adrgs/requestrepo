from dnslib import DNSRecord, QTYPE, RD, RR
from dnslib import A, AAAA, CNAME, MX, NS, SOA, TXT
from typing import TypedDict
import datetime
import sys

if sys.version_info < (3, 11):
    from typing_extensions import NotRequired
else:
    from typing import NotRequired


class DnsEntry(TypedDict):
    domain: str
    type: str
    value: str
    _id: str


class DnsRequestLog(TypedDict):
    type: str
    date: int
    ip: str
    port: int
    country: NotRequired[str]
    dtype: str
    name: str
    uid: str
    reply: str
    raw: str
    _id: str


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
        return None

    def as_rr(self, alt_rname) -> RR:
        return RR(rname=self._rname or alt_rname, rtype=self._rtype, **self.kwargs)

    def sensible_ttl(self) -> int:
        return 1

    @property
    def is_soa(self) -> bool:
        return self._rtype == QTYPE.SOA

    def __str__(self) -> str:
        return "{} {}".format(QTYPE[self._rtype], self.kwargs)
