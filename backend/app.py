# app.py
from flask import Flask, jsonify, request, make_response, redirect, send_from_directory
from werkzeug.routing import Rule
from mongolog import *
import base64
import random
import datetime
import jwt
from util import get_random_string
import re
import json
import os
import sys
from pprint import pprint

#
#   TODO: fix X-Real-IP nginx https://docs.nginx.com/nginx/admin-guide/web-server/reverse-proxy/
#
#

class CustomServerHeaderFlask(Flask):
    def process_response(self, response):
        response.headers['Server'] = 'requestrepo.com'
        super(CustomServerHeaderFlask, self).process_response(response)
        return(response)

app = CustomServerHeaderFlask(__name__, static_url_path='/public/static')
app.url_map.add(Rule('/', endpoint='index'))
app.url_map.add(Rule('/<path:path>', endpoint='catch_all'))

JWT_SECRET = 'changethis'
REGXPRESSION = '(\\.|^)[0-9a-z]{8}\\.requestrepo\\.com(:[0-9]+)?$'

def verify_jwt(token):
    try:
        if token:
            return jwt.decode(token, JWT_SECRET, algorithms=['HS256'])['subdomain']
    except:
        return None
    return None

def write_basic_file(subdomain):
    file_data = {'headers':[{'header':'Access-Control-Allow-Origin','value':'*'},{'header':'Content-Type','value':'text/html'}], 'status_code':200, 'raw':''}

    with open('pages/'+subdomain, 'w') as outfile:
        json.dump(file_data, outfile)

def log_request(request, subdomain):
    dic = {}
    headers = dict(request.headers)

    dic['raw']       = request.stream.read()
    dic['uid']       = subdomain
    if 'Requestrepo-X-Forwarded-For' in headers:
        dic['ip']    = headers['Requestrepo-X-Forwarded-For']
        del headers['Requestrepo-X-Forwarded-For']
    else:
        dic['ip']    = request.remote_addr
    dic['headers']   = headers
    dic['method']    = request.method
    dic['protocol']  = request.environ.get('SERVER_PROTOCOL')
    if request.full_path[-1] == '?' and request.url[-1] != '?':
        dic['path']  = request.full_path[:-1]
    else:
        dic['path']  = request.full_path
    if dic['path'].find('?') > -1:
        dic['query'] = dic['path'][dic['path'].find('?'):]
    else:
        dic['query'] = ''
    dic['url']       = request.url
    dic['date']      = datetime.datetime.utcnow()

    http_insert_into_db(dic)

def get_subdomain_from_hostname(host):
    subdomain = re.search(REGXPRESSION, host)
    if subdomain == None:
        return ""
    else:
        subdomain = subdomain.group(0)
        if subdomain[0] == '.':
            subdomain = subdomain[1:9]
        else:
            subdomain = subdomain[:8]
    return subdomain

def subdomain_response(request, subdomain):
    log_request(request, subdomain)
    data = {'raw':'', 'headers':[], 'status_code':200}
    if not os.path.exists('pages/'+subdomain):
        write_basic_file(subdomain)
    with open('pages/'+subdomain,'r') as json_file:
        try:
            data = json.load(json_file)
        except:
            pass
    try:
        resp = make_response(base64.b64decode(data['raw']))
    except:
        resp = make_response('')
    resp.headers['server'] = 'requestrepo.com'
    if 'headers' in data:
        for header in data['headers']:
            resp.headers[header['header']] = header['value']
    resp.status_code = data['status_code']
    return resp


@app.endpoint('index')
def index():
    subdomain = get_subdomain_from_hostname(request.host)

    if request.host.count('.') != 1 and len(subdomain) == 8:
        return subdomain_response(request, subdomain)
    else:
        subdomain = verify_jwt(request.cookies.get('token'))
        return send_from_directory('public', 'index.html')
        #if subdomain:
        #    return "your url is " + subdomain + ".requestrepo.com"
        #return "You don't have an URL"

@app.endpoint('catch_all')
def catch_all(path):
    if request.host.count('.') == 1:
        return send_from_directory('public', path)

    subdomain = get_subdomain_from_hostname(request.host)
    if request.host.count('.') != 1 and len(subdomain) == 8:
        return subdomain_response(request, subdomain)
    else:
        #if path=='':
        #    path = 'index.html'
        #return send_from_directory('public', path)
        rindex = request.host.rfind(':')
        port = ':80'
        nr = request.host[rindex+1:]
        if nr.isdigit():
            if int(nr) <= 20000:
                port = ':' + nr
        return redirect("//requestrepo.com" + port + "/", code=302)


@app.route('/api/get_dns_requests')
def get_dns_requests():
    subdomain = get_subdomain_from_hostname(request.host)
    if request.host.count('.') != 1 and len(subdomain) == 8:
        return subdomain_response(request, subdomain)
    else:
        subdomain = verify_jwt(request.cookies.get('token'))
        time = request.args.get('t')
        if subdomain:
            return jsonify(dns_get_subdomain(subdomain, time))
        else:
            return jsonify({'err':'Unauthorized'}), 401

@app.route('/api/get_http_requests')
def get_http_requests():
    subdomain = get_subdomain_from_hostname(request.host)
    if request.host.count('.') != 1 and len(subdomain) == 8:
        return subdomain_response(request, subdomain)
    else:
        subdomain = verify_jwt(request.cookies.get('token'))
        time = request.args.get('t')
        if subdomain:
            return jsonify(http_get_subdomain(subdomain, time))
            #return jsonify(dns_get_subdomain(subdomain, time))
        else:
            return jsonify({'err':'Unauthorized'}), 401

@app.route('/api/get_requests')
def get_requests():
    subdomain = get_subdomain_from_hostname(request.host)
    if request.host.count('.') != 1 and len(subdomain) == 8:
        return subdomain_response(request, subdomain)
    else:
        subdomain = verify_jwt(request.cookies.get('token'))
        time = request.args.get('t')
        if subdomain:
            http_requests = http_get_subdomain(subdomain, time)
            dns_requests = dns_get_subdomain(subdomain, time)
            server_time = datetime.datetime.utcnow().strftime('%s')
            return jsonify({
                'http': http_requests,
                'dns': dns_requests,
                'date': server_time
            })
        else:
            return jsonify({'err':'Unauthorized'}), 401

@app.route('/api/get_token', methods=['POST','OPTIONS'])
def get_token():
    subdomain = get_subdomain_from_hostname(request.host)
    if request.host.count('.') != 1 and len(subdomain) == 8:
        return subdomain_response(request, subdomain)
    else:
        if request.method == 'OPTIONS':
            return 'POST'

        subdomain = get_random_string(8)
        while users_get_subdomain(subdomain) != None:
            subdomain = get_random_string(8)
        dns_delete_records(subdomain)

        write_basic_file(subdomain)

        payload = {
            'iat': datetime.datetime.utcnow(),
            'exp': datetime.datetime.utcnow() + datetime.timedelta(days=31),
            'subdomain': subdomain
        }
        token = jwt.encode(payload,JWT_SECRET,algorithm='HS256')
        resp = make_response(token)
        resp.set_cookie('token', token)
        return resp

@app.route('/api/get_server_time')
def get_server_time():
    subdomain = get_subdomain_from_hostname(request.host)
    if request.host.count('.') != 1 and len(subdomain) == 8:
        return subdomain_response(request, subdomain)
    else:
        return jsonify({'date':datetime.datetime.utcnow().strftime('%s')})

@app.route('/api/delete_request', methods=['POST'])
def delete_request():
    subdomain = get_subdomain_from_hostname(request.host)
    if request.host.count('.') != 1 and len(subdomain) == 8:
        return subdomain_response(request, subdomain)
    else:
        subdomain = verify_jwt(request.cookies.get('token'))
        if subdomain:
            content = request.json
            if content:
                _id = content.get('id')
                rtype = content.get('type')
                delete_request_from_db(_id, subdomain, rtype)
                return jsonify({"rtype":rtype, "_id":_id})
        return jsonify("error")

@app.route('/api/get_file', methods=['GET'])
def get_file():
    subdomain = get_subdomain_from_hostname(request.host)
    if request.host.count('.') != 1 and len(subdomain) == 8:
        return subdomain_response(request, subdomain)
    subdomain = verify_jwt(request.cookies.get('token'))
    if subdomain:
        with open('pages/'+subdomain, 'r') as outfile:
            return outfile.read()
    else:
        return '{"raw":"", "headers":[], status_code:200}'

@app.route('/api/update_file', methods=['POST'])
def update_file():
    subdomain = get_subdomain_from_hostname(request.host)
    if request.host.count('.') != 1 and len(subdomain) == 8:
        return subdomain_response(request, subdomain)
    subdomain = verify_jwt(request.cookies.get('token'))
    if subdomain:
        content = request.json
        status_code = 200
        if 'status_code' in content:
            try:
                try:
                    if len(content['status_code'])>9:
                        return '{"error":"invalid status_code"}'
                    status_code = int(content['status_code'])
                except:
                    pass
            except:
                return '{"error":"invalid status_code"}'
        raw = ""
        if 'raw' in content:
            if len(content['raw']) <= 2000000:
                try:
                    base64.b64decode(content['raw'])
                    raw = content['raw']
                except:
                    return '{"error":"invalid response"}'
            else:
                return '{"error":"response should be smaller than 2MB"}'
        headers=[]
        if 'headers' in content:
            if len(headers) <= 30:
                for header in content['headers']:
                    if 'header' in header and 'value' in header:
                        #if 'content-length' in header['header'].lower():
                        #    continue
                        headers.append({'header': header['header'], 'value':header['value']})
            else:
                return '{"error":"maximum of 30 headers"}'
            with open('pages/'+subdomain, 'w') as outfile:
                json.dump({'headers':headers, 'raw':raw, 'status_code': status_code}, outfile)
        return '{"msg":"Updated response"}'
    return '{"error":"unauthenticated"}'

@app.route('/api/get_dns_records', methods=['GET'])
def get_dns_records():
    subdomain = get_subdomain_from_hostname(request.host)
    if request.host.count('.') != 1 and len(subdomain) == 8:
        return subdomain_response(request, subdomain)
    subdomain = verify_jwt(request.cookies.get('token'))
    if subdomain:
        return jsonify(dns_get_records(subdomain))
    return '{"error":"unauthenticated"}'

DNS_RECORDS = ['A', 'AAAA', 'CNAME', 'TXT']

@app.route('/api/update_dns_records', methods=['POST'])
def update_dns_records():
    subdomain = get_subdomain_from_hostname(request.host)
    if request.host.count('.') != 1 and len(subdomain) == 8:
        return subdomain_response(request, subdomain)
    subdomain = verify_jwt(request.cookies.get('token'))
    if subdomain:
        dns_delete_records(subdomain)
        content = request.json
        if 'records' in content:
            for record in content['records']:
                if type(record) is not dict:
                    continue
                domain = record.get('domain')
                dtype = record.get('type')
                value = record.get('value')
                if domain is None or dtype is None or value is None:
                    continue
                if domain == "" or value == "":
                    continue
                domain=domain.lower()
                try:
                    if len(domain) > 63:
                        return '{"error":"Domain too big"}'
                        continue
                    if len(value) > 255:
                        return '{"error":"Value too big"}'
                        continue
                    if type(dtype) is not int:
                        return '{"error":"Invalid type"}'
                        continue
                    if dtype < 0 or dtype >= len(DNS_RECORDS):
                        return '{"error":"Invalid type range"}'
                        continue
                    if not re.search("^[ -~]+$", value):
                        return '{"error":"Invailid regex1"}'
                        continue
                    if not re.match("^[A-Za-z0-9](?:[A-Za-z0-9\\-_\\.]{0,61}[A-Za-z0-9])?$", domain):
                        return '{"error":"invalid regex2"}'
                        continue
                    domain = domain + '.' + subdomain + '.requestrepo.com.'
                    dtype = DNS_RECORDS[dtype]
                    dns_insert_record(subdomain, domain, dtype, value)
                except Exception as e:
                    return '{"error":"'+str(e)+'"}'
                    continue
            return '{"msg":"Updated records"}'
        return '{"error":"Invalid records"}'

    return '{"error":"unauthenticated"}'

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=21337, debug=True)
    #app.run(host='0.0.0.0', port=21337, debug=False)
