from pymongo import MongoClient
from config import config
from utils import get_subdomain

client = MongoClient(
    "mongodb://%s:%s@%s"
    % (config.mongodb_username, config.mongodb_password, config.mongodb_hostname),
    27017,
)


def insert_into_db(value: dict[str, any]) -> None:
    db = client[config.mongodb_database]

    collection = db["dns_requests"]
    value["_deleted"] = False
    collection.insert_one(value)


def get_dns_record(domain: str, dtype: str) -> any | None:
    db = client[config.mongodb_database]

    ddns = db["ddns"]
    result = ddns.find_one({"domain": domain, "type": dtype})
    return result


def update_dns_record(
    subdomain: str | None, domain: str, dtype: str, newval: str
) -> None:
    db = client[config.mongodb_database]

    ddns = db["ddns"]

    if not subdomain:
        subdomain = get_subdomain(domain)

    if not subdomain:
        return

    ddns.update_one(
        {"subdomain": subdomain, "domain": domain, "type": dtype},
        {"$set": {"value": newval}},
    )
