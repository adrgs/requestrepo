import os
from pymongo import MongoClient
import urllib.parse
import re
from config import config

client = MongoClient(
    "mongodb://%s:%s@%s"
    % (config.mongodb_username, config.mongodb_password, config.mongodb_hostname),
    27017,
)

REGXPRESSION = "^(.*)(\\.?[0-9a-z]{8}\\.requestrepo\\.com\\.?)$"


def insert_into_db(value):
    db = client[config.mongodb_database]

    collection = db["dns_requests"]
    value["_deleted"] = False
    collection.insert_one(value)


def get_dns_record(domain, dtype):
    db = client[config.mongodb_database]

    ddns = db["ddns"]
    result = ddns.find_one({"domain": domain, "type": dtype})
    return result


def update_dns_record(subdomain, domain, dtype, newval):
    db = client[config.mongodb_database]

    ddns = db["ddns"]
    if subdomain == None:
        uid = re.search(REGXPRESSION, domain)
        if uid == None:
            uid = "Bad"
        else:
            uid = uid.group(2)
            if uid[0] == ".":
                subdomain = uid[1:9]
            else:
                subdomain = uid[:8]
    ddns.update_one(
        {"subdomain": subdomain, "domain": domain, "type": dtype},
        {"$set": {"value": newval}},
    )
