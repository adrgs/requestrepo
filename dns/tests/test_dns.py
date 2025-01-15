import pytest
from utils import get_subdomain
from ns import Resolver, get_dns_record, insert_into_db, update_dns_record, save_into_db
from dnslib import DNSRecord, DNSHeader, DNSQuestion, QTYPE
from unittest.mock import Mock, patch, MagicMock
import json
import redis
from models import DnsRequestLog, DnsEntry
import base64

# Setup mock Redis
mock_redis = MagicMock(spec=redis.Redis)
mock_redis.get.return_value = None
mock_redis.set.return_value = True
mock_redis.delete.return_value = True
mock_redis.rpush.return_value = 1
mock_redis.publish.return_value = 1

@pytest.fixture
def mock_redis_connection():
    with patch('redis.Redis', return_value=mock_redis) as mock:
        yield mock

@pytest.fixture
def resolver():
    return Resolver(server_ip="1.2.3.4", server_domain="localhost")

@pytest.fixture
def mock_handler():
    handler = Mock()
    handler.client_address = ("1.2.3.4", 53)  # (ip, port)
    handler.request = [b"test data"]  # Raw request data
    return handler

def test_get_subdomain_valid():
    # Test valid subdomain extraction
    assert get_subdomain("abcd1234.localhost.") == "abcd1234"
    assert get_subdomain("test.abcd1234.localhost.") == "abcd1234"
    assert get_subdomain("something.test.abcd1234.localhost.") == "abcd1234"

def test_get_subdomain_invalid():
    # Test invalid cases
    assert get_subdomain("invalid.com.") is None
    assert get_subdomain("toolong123456.localhost.") is None
    assert get_subdomain("invalid#chars.localhost.") is None
    assert get_subdomain("short.localhost.") is None

def test_get_dns_record(mock_redis_connection):
    # Test getting DNS records
    mock_redis.get.return_value = json.dumps(["1.2.3.4"])
    
    result = get_dns_record("test.localhost.", "A")
    assert result == ["1.2.3.4"]
    mock_redis.get.assert_called_with("dns:A:test.localhost.")

    # Test non-existent record
    mock_redis.get.return_value = None
    result = get_dns_record("nonexistent.localhost.", "A")
    assert result is None

def test_update_dns_record(mock_redis_connection):
    # Test updating DNS records
    domain = "test.localhost."
    dtype = "A"
    value = "1.2.3.4"

    # Test new record
    mock_redis.get.return_value = None
    update_dns_record(domain, dtype, value)
    
    mock_redis.set.assert_called()
    call_args = mock_redis.set.call_args[0]
    assert call_args[0] == f"dns:{dtype}:{domain}"
    stored_data = json.loads(call_args[1])
    assert stored_data["domain"] == domain
    assert stored_data["type"] == dtype
    assert stored_data["value"] == value

    # Test updating existing record
    existing_record = {
        "domain": domain,
        "type": dtype,
        "value": "5.6.7.8",
        "_id": "existing-id"
    }
    mock_redis.get.return_value = json.dumps(existing_record)
    
    update_dns_record(domain, dtype, value)
    call_args = mock_redis.set.call_args[0]
    stored_data = json.loads(call_args[1])
    assert stored_data["_id"] == "existing-id"  # Should keep existing ID

def test_insert_into_db(mock_redis_connection):
    # Test inserting DNS request logs
    test_log = DnsRequestLog(
        type="dns",
        date=1234567890,
        ip="1.2.3.4",
        port=53,
        dtype="A",
        name="test.localhost.",
        uid="abcd1234",
        reply="test reply",
        raw=base64.b64encode(b"test data").decode(),
        _id="test-id"
    )

    insert_into_db(test_log)

    # Verify Redis calls
    mock_redis.publish.assert_called_with("pubsub:abcd1234", json.dumps(test_log))
    mock_redis.rpush.assert_called_with("requests:abcd1234", json.dumps(test_log))
    mock_redis.set.assert_called_with("request:abcd1234:test-id", 0, ex=604800)  # idx is 1-1=0

def test_resolver_a_record(resolver, mock_handler):
    request = DNSRecord(DNSHeader(qr=0, aa=1), q=DNSQuestion("test.localhost", QTYPE.A))
    
    with patch('dns.ns.get_dns_record') as mock_get_dns:
        # Test default A record
        mock_get_dns.return_value = None
        reply = resolver.resolve(request, mock_handler)
        assert len(reply.rr) == 1
        assert str(reply.rr[0].rdata) == "1.2.3.4"

        # Test custom A record
        mock_get_dns.return_value = ["5.6.7.8"]
        reply = resolver.resolve(request, mock_handler)
        assert len(reply.rr) == 1
        assert str(reply.rr[0].rdata) == "5.6.7.8"

def test_resolver_txt_record(resolver, mock_handler):
    request = DNSRecord(DNSHeader(qr=0, aa=1), q=DNSQuestion("test.localhost", QTYPE.TXT))
    
    with patch('dns.ns.get_dns_record') as mock_get_dns:
        # Test custom TXT record
        mock_get_dns.return_value = ["test-txt-record"]
        reply = resolver.resolve(request, mock_handler)
        assert len(reply.rr) == 1
        assert str(reply.rr[0].rdata) == '"test-txt-record"'

def test_resolver_cname_record(resolver, mock_handler):
    request = DNSRecord(DNSHeader(qr=0, aa=1), q=DNSQuestion("test.localhost", QTYPE.CNAME))
    
    with patch('dns.ns.get_dns_record') as mock_get_dns:
        # Test default CNAME record
        mock_get_dns.return_value = None
        reply = resolver.resolve(request, mock_handler)
        assert len(reply.rr) == 1
        assert str(reply.rr[0].rdata) == "localhost."

        # Test custom CNAME record
        mock_get_dns.return_value = ["custom.localhost."]
        reply = resolver.resolve(request, mock_handler)
        assert len(reply.rr) == 1
        assert str(reply.rr[0].rdata) == "custom.localhost."

def test_save_into_db(mock_redis_connection, mock_handler):
    request = DNSRecord(DNSHeader(qr=0, aa=1), q=DNSQuestion("abcd1234.localhost", QTYPE.A))
    reply = Resolver("1.2.3.4", "localhost").resolve(request, mock_handler)
    
    with patch('ip2country.ip_to_country', return_value="US"):
        save_into_db(reply, "1.2.3.4", 53, b"test data")
        
        # Verify Redis calls for logging
        mock_redis.publish.assert_called()
        mock_redis.rpush.assert_called()
        mock_redis.set.assert_called()
        
        # Verify the published data structure
        call_args = mock_redis.publish.call_args[0]
        assert call_args[0] == "pubsub:abcd1234"
        published_data = json.loads(call_args[1])
        assert published_data["type"] == "dns"
        assert published_data["ip"] == "1.2.3.4"
        assert published_data["port"] == 53
        assert published_data["country"] == "US"
        assert published_data["dtype"] == "A"
        assert published_data["uid"] == "abcd1234"