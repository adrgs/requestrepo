import os
from pymongo import MongoClient
import urllib.parse
import re

if 'MONGODB_DATABASE' in os.environ:
    MONGODB_DATABASE = os.environ['MONGODB_DATABASE']
else:
    MONGODB_DATABASE = 'requestrepo'

if 'MONGODB_USERNAME' in os.environ:
    MONGODB_USERNAME = os.environ['MONGODB_USERNAME']
else:
    MONGODB_USERNAME = 'requestrepouser'

if 'MONGODB_PASSWORD' in os.environ:
    MONGODB_PASSWORD = os.environ['MONGODB_PASSWORD']
else:
    MONGODB_PASSWORD = 'changethis'

if 'MONGODB_HOSTNAME' in os.environ:
    MONGODB_HOSTNAME = os.environ['MONGODB_HOSTNAME']
else:
    MONGODB_HOSTNAME = '127.0.0.1'

username = urllib.parse.quote_plus(MONGODB_USERNAME)
password = urllib.parse.quote_plus(MONGODB_PASSWORD)

def insert_into_db(value):
    client = MongoClient('mongodb://%s:%s@%s' % (username, password, MONGODB_HOSTNAME), 27017)
    db = client[MONGODB_DATABASE]

    collection = db['dns_requests']
    value['_deleted'] = False
    collection.insert_one(value)
    client.close()


def get_dns_record(domain, dtype):
    client = MongoClient('mongodb://%s:%s@%s' % (username, password, MONGODB_HOSTNAME), 27017)
    db = client[MONGODB_DATABASE]

    ddns = db['ddns']
    result = ddns.find_one({'domain':domain, 'type':dtype})
    client.close()
    return result



#REGXPRESSION = '^\\.?[0-9a-z]{8}\\.requestrepo\\.com\\.?$'
REGXPRESSION = '^(.*)(\\.?[0-9a-z]{8}\\.requestrepo\\.com\\.?)$'
def update_dns_record(subdomain, domain, dtype, newval):
    client = MongoClient('mongodb://%s:%s@%s' % (username, password, MONGODB_HOSTNAME), 27017)
    db = client[MONGODB_DATABASE]

    ddns = db['ddns']
    if subdomain == None:
        uid = re.search(REGXPRESSION, domain)
        if uid == None:
            uid = "Bad"
        else:
            uid = uid.group(2)
            if uid[0] == '.':
                subdomain = uid[1:9]
            else:
                subdomain = uid[:8]
    ddns.update_one({'subdomain':subdomain, 'domain':domain, 'type':dtype}, {'$set':{'value':newval}})
    client.close()

#def insert_dns_record(subdomain, domain, dtype, val):
#    ddns.insert_one({'subdomain':subdomain, 'domain':domain, 'type':dtype, 'value':val})
