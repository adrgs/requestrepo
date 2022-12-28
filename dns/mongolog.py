import os
from pymongo import MongoClient
import urllib.parse

if 'MONGODB_DATABASE' in os.environ:
    MONGODB_DATABASE = os.environ['MONGODB_DATABASE']
else:
    MONGODB_DATABASE = 'requestrepo'

if 'MONGODB_USERNAME' in os.environ:
    MONGODB_USERNAME = os.environ['MONGODB_USERNAME']
else:
    MONGODB_USERNAME = 'root'

if 'MONGODB_PASSWORD' in os.environ:
    MONGODB_PASSWORD = os.environ['MONGODB_PASSWORD']
else:
    MONGODB_PASSWORD = 'rootpassword'

if 'MONGODB_HOSTNAME' in os.environ:
    MONGODB_HOSTNAME = os.environ['MONGODB_HOSTNAME']
else:
    MONGODB_HOSTNAME = '127.0.0.1'

username = urllib.parse.quote_plus(MONGODB_USERNAME)
password = urllib.parse.quote_plus(MONGODB_PASSWORD)

client = MongoClient('mongodb://%s:%s@%s' % (username, password, MONGODB_HOSTNAME), 27017)
db = client[MONGODB_DATABASE]

collection = db['dns_requests']
ddns = db['ddns']

def insert_into_db(value):
    value['_deleted'] = False
    collection.insert_one(value)

def get_from_db():
    return collection.find({'_deleted':False}, {'_deleted':False})

def get_dns_record(domain, dtype):
    return ddns.find_one({'domain':domain, 'type':dtype})



#REGXPRESSION = '^\\.?[0-9a-z]{8}\\.requestrepo\\.com\\.?$'
REGXPRESSION = '^(.*)(\\.?[0-9a-z]{8}\\.requestrepo\\.com\\.?)$'
def update_dns_record(subdomain, domain, dtype, newval):
    if subdomain == None:
        uid = re.search(REGXPRESSION, name)
        if uid == None:
            uid = "Bad"
        else:
            uid = uid.group(2)
            if uid[0] == '.':
                subdomain = uid[1:9]
            else:
                subdomain = uid[:8]
    ddns.update_one({'subdomain':subdomain, 'domain':domain, 'type':dtype}, {'$set':{'value':newval}})

def insert_dns_record(subdomain, domain, dtype, val):
    ddns.insert_one({'subdomain':subdomain, 'domain':domain, 'type':dtype, 'value':val})
