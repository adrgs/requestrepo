import os
import pymongo
from bson.objectid import ObjectId
import urllib.parse
import base64
from config import config

client = pymongo.MongoClient(
    "mongodb://%s:%s@%s"
    % (config.mongodb_username, config.mongodb_password, config.mongodb_hostname),
    27017,
)

db = client[config.mongodb_database]

# Databases
collection = db["dns_requests"]
ddns = db["ddns"]
users = db["users"]
http = db["http"]

# create indexes
collection.create_index([("uid", 1), ("_deleted", 1), ("date", 1)], background=True)
http.create_index([("uid", 1), ("_deleted", 1), ("date", 1)], background=True)


def dns_insert_into_db(value):
    value["_deleted"] = False
    collection.insert_one(value)


def dns_get_from_db():
    return collection.find({"_deleted": False}, {"_deleted": False})


def dns_get_records(subdomain):
    l = []
    for x in ddns.find({"subdomain": subdomain}):
        x["_id"] = str(x["_id"])
        l.append(x)
    return l


def dns_delete_records(subdomain):
    ddns.delete_many({"subdomain": subdomain})


def dns_insert_record(subdomain, domain, dtype, val):
    ddns.insert_one(
        {"subdomain": subdomain, "domain": domain, "type": dtype, "value": val}
    )


def dns_get_subdomain(subdomain, time):
    l = []

    find = {"uid": subdomain, "_deleted": False}
    try:
        if time != None:
            find["date"] = {"$gte": time}
    except:
        pass

    for x in collection.find(find, {"_deleted": False}):
        x["_id"] = str(x["_id"])
        x["raw"] = str(base64.b64encode(x["raw"]), "utf-8")
        l.append(x)

    return l


def dns_delete_records(subdomain):
    ddns.delete_many({"subdomain": subdomain})


def dns_delete_request(_id, subdomain):
    collection.update_one(
        {"uid": subdomain, "_id": ObjectId(_id)}, {"$set": {"_deleted": True}}
    )


def http_insert_into_db(dic):
    dic["_deleted"] = False
    http.insert_one(dic)


def http_get_from_db():
    l = []
    for x in http.find({"_deleted": False}):
        x["_id"] = str(x["_id"])
        x["raw"] = str(base64.b64encode(x["raw"]), "utf-8")
        l.append(x)
    return l


def http_get_subdomain(subdomain, time):
    l = []

    find = {"uid": subdomain, "_deleted": False}
    try:
        if time != None:
            find["date"] = {"$gte": time}
    except:
        pass

    for x in http.find(find, {"_deleted": False}):
        x["_id"] = str(x["_id"])
        x["raw"] = str(base64.b64encode(x["raw"]), "utf-8")
        l.append(x)

    return l


def http_delete_request(_id, subdomain):
    http.update_one(
        {"_id": ObjectId(_id), "uid": subdomain}, {"$set": {"_deleted": True}}
    )


def users_insert_into_db(ip, subdomain):
    collection.insert_one({"ip": ip, "subdomain": subdomain})


def users_get_subdomain(subdomain):
    return users.find_one({"subdomain": subdomain})


def delete_request_from_db(_id, subdomain, dtype):
    if dtype == "HTTP":
        http_delete_request(_id, subdomain)
    elif dtype == "DNS":
        dns_delete_request(_id, subdomain)
