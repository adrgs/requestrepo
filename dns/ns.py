#!/usr/bin/env python3
import sys
import datetime
import time
import os
from time import sleep
import re
import random

from dnslib import DNSLabel, QTYPE, RD, RR, RCODE
from dnslib import A, AAAA, CNAME, MX, NS, SOA, TXT
from dnslib.server import DNSServer
from mongolog import insert_into_db, update_dns_record, get_dns_record

EPOCH = datetime.datetime(1970, 1, 1)
SERIAL = int(datetime.datetime.now(datetime.timezone.utc).timestamp())

TYPE_LOOKUP = {
    A: QTYPE.A,
    AAAA: QTYPE.AAAA,
    CNAME: QTYPE.CNAME,
    MX: QTYPE.MX,
    NS: QTYPE.NS,
    SOA: QTYPE.SOA,
    TXT: QTYPE.TXT,
}

SUSPICIOUS_RECORDS = {
    'ftp': 'FTP ',
    'cpanel': 'cPanel hosting control panel',
    'admin': 'common record for admin control panels',
    'ssh': 'SSH service',
    'wordpress': 'common blogging platform',
    'store': 'common record for stores',
    'staging': 'common record for staging environments',
    'mail': 'common record for email',
}


class Record:
    def __init__(self,
                 rdata_type,
                 *args,
                 rtype=None,
                 rname=None,
                 ttl=None,
                 **kwargs):
        if isinstance(rdata_type, RD):
            # actually an instance, not a type
            self._rtype = TYPE_LOOKUP[rdata_type.__class__]
            rdata = rdata_type
        else:
            self._rtype = TYPE_LOOKUP[rdata_type]
            if rdata_type == SOA and len(args) == 2:
                # add sensible times to SOA
                args += (
                    (
                        SERIAL,  # serial number
                        60 * 60 * 1,  # refresh
                        60 * 60 * 3,  # retry
                        60 * 60 * 24,  # expire
                        60 * 60 * 1,  # minimum
                    ), )
            rdata = rdata_type(*args)

        if rtype:
            self._rtype = rtype
        self._rname = rname
        self.kwargs = dict(rdata=rdata,
                           ttl=self.sensible_ttl() if ttl is None else ttl,
                           **kwargs)

    def try_rr(self, q):
        if q.qtype == QTYPE.ANY or q.qtype == self._rtype:
            return self.as_rr(q.qname)

    def as_rr(self, alt_rname):
        return RR(rname=self._rname or alt_rname,
                  rtype=self._rtype,
                  **self.kwargs)

    def sensible_ttl(self):
        return 1
        #if self._rtype in (QTYPE.NS, QTYPE.SOA):
        #    return 60 * 60 * 24
        #else:
        #    return 300

    @property
    def is_soa(self):
        return self._rtype == QTYPE.SOA

    def __str__(self):
        return '{} {}'.format(QTYPE[self._rtype], self.kwargs)


if 'SERVER_IP' in os.environ:
    SERVER_IP = os.environ['SERVER_IP']
else:
    SERVER_IP = '127.0.0.1'

#REGXPRESSION = '^\\.?[0-9a-z]{8}\\.requestrepo\\.com\\.?$'
REGXPRESSION = '^(.+\\.)?(([0-9a-z]{8})\\.requestrepo\\.com\\.?)$'


def save_into_db(reply, ip, raw):
    name = str(reply.q.qname)
    uid = re.search(REGXPRESSION, name.lower())
    if uid == None:
        uid = "Bad"
    else:
        uid = uid.group(3)
        if uid[0] == '.':
            uid = uid[1:9]
        else:
            uid = uid[:8]

    data = {
        "date": int(datetime.datetime.now(datetime.timezone.utc).timestamp()),
        "ip": ip,
        "type": QTYPE[reply.q.qtype],
        "name": name,
        "uid": uid,
        "reply": str(reply),
        "raw": raw
    }
    insert_into_db(data)


class Resolver:
    def __init__(self):
        self.server_ip = SERVER_IP

    def resolve(self, request, handler):
        reply = request.reply()

        # We assume that the data in the DB is correct (using server side checks)
        new_record = None

        if QTYPE[reply.q.qtype] == 'CNAME':
            data = get_dns_record(str(reply.q.qname), 'CNAME')
            if data == None:
                new_record = Record(CNAME, 'requestrepo.com.')
            else:
                new_record = Record(CNAME, data['value'])
        elif QTYPE[reply.q.qtype] == 'TXT':
            data = get_dns_record(str(reply.q.qname), 'TXT')
            if data == None:
                new_record = Record(TXT, os.getenv('TXT') or 'Hello!')
            else:
                new_record = Record(TXT, data['value'])
        elif QTYPE[reply.q.qtype] == 'A':
            data = get_dns_record(str(reply.q.qname), 'A')
            if data == None:
                new_record = Record(A, self.server_ip)
            else:
                ips = data['value']
                if '/' not in ips and '%' not in ips:
                    new_record = Record(A, ips)
                else:
                    if '%' in ips:
                        ips = ips.split('%')
                        idx = random.randint(0, len(ips) - 1)
                        if '/' in ips[idx]:
                            new_ips = ips[idx].split('/')
                            new_record = Record(A, new_ips[0])
                            new_ips = '/'.join(new_ips[1:] + [new_ips[0]])
                            ips[idx] = new_ips
                            ips = '%'.join(ips)
                            update_dns_record(data['subdomain'],
                                              data['domain'], 'A', ips)
                        else:
                            new_record = Record(A, ips[idx])
                    else:
                        ips = ips.split('/')
                        new_record = Record(A, ips[0])
                        ips = '/'.join(ips[1:] + [ips[0]])
                        update_dns_record(data['subdomain'], data['domain'],
                                          'A', ips)
        elif QTYPE[reply.q.qtype] == 'AAAA':
            data = get_dns_record(str(reply.q.qname), 'AAAA')
            if data == None:
                try:
                    new_record = Record(AAAA, self.server_ip)
                except:
                    pass
            else:
                ips = data['value']
                if '/' not in ips and '%' not in ips:
                    new_record = Record(AAAA, ips)
                else:
                    if '%' in ips:
                        ips = ips.split('%')
                        idx = random.randint(0, len(ips) - 1)
                        if '/' in ips[idx]:
                            new_ips = ips[idx].split('/')
                            new_record = Record(AAAA, new_ips[0])
                            new_ips = '/'.join(new_ips[1:] + [new_ips[0]])
                            ips[idx] = new_ips
                            ips = '%'.join(ips)
                            update_dns_record(data['subdomain'],
                                              str(reply.q.qname), 'AAAA', ips)
                        else:
                            new_record = Record(AAAA, ips[idx])
                    else:
                        ips = ips.split('/')
                        new_record = Record(AAAA, ips[0])
                        ips = '/'.join(ips[1:] + [ips[0]])
                        update_dns_record(data['subdomain'],
                                          str(reply.q.qname), 'AAAA', ips)

        if new_record != None:
            reply.add_answer(new_record.try_rr(request.q))
            try:
                save_into_db(reply, handler.client_address[0],
                             handler.request[0])
            except Exception as ex:
                print(ex)
                pass

        return reply


resolver = Resolver()
servers = [
    DNSServer(resolver, port=53, address='0.0.0.0', tcp=True),
    DNSServer(resolver, port=53, address='0.0.0.0', tcp=False),
]

if __name__ == '__main__':
    for s in servers:
        s.start_thread()

    try:
        while 1:
            sleep(0.1)
    except KeyboardInterrupt:
        pass
    finally:
        for s in servers:
            s.stop()
