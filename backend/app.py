from flask import Flask, jsonify, request, make_response, send_from_directory
from werkzeug.routing import Rule
from mongolog import *
import base64
import datetime
import jwt
from util import get_random_subdomain
import re
import json
import os

JWT_SECRET = os.getenv('JWT_SECRET', os.urandom(32))
DOMAIN = os.getenv('DOMAIN', 'requestrepo.com')

app = Flask(__name__, static_url_path='/public/static')
app.url_map.add(Rule('/', endpoint='index'))
app.url_map.add(Rule('/<path:path>', endpoint='catch_all'))


def verify_jwt(token):
    try:
        if token:
            return jwt.decode(token, JWT_SECRET,
                              algorithms=['HS256'])['subdomain']
    except:
        return None
    return None


def write_basic_file(subdomain):
    file_data = {
        'headers': [{
            'header': 'Access-Control-Allow-Origin',
            'value': '*'
        }, {
            'header': 'Content-Type',
            'value': 'text/html'
        }],
        'status_code':
        200,
        'raw':
        ''
    }

    with open('pages/' + subdomain, 'w') as outfile:
        json.dump(file_data, outfile)


def log_request(request, subdomain):
    dic = {}
    headers = dict(request.headers)

    dic['raw'] = request.stream.read()
    dic['uid'] = subdomain
    dic['ip'] = request.remote_addr
    dic['headers'] = headers
    dic['method'] = request.method
    dic['protocol'] = request.environ.get('SERVER_PROTOCOL')
    if request.full_path[-1] == '?' and request.url[-1] != '?':
        dic['path'] = request.full_path[:-1]
    else:
        dic['path'] = request.full_path
    if dic['path'].find('?') > -1:
        dic['query'] = dic['path'][dic['path'].find('?'):]
    else:
        dic['query'] = ''
    dic['url'] = request.url
    dic['date'] = datetime.datetime.utcnow()

    http_insert_into_db(dic)


def get_subdomain_from_hostname(host):
    subdomain = host[:-len(DOMAIN) - 1][-8:]
    if not subdomain or not subdomain.isalnum():
        return None

    return subdomain.lower()


def subdomain_response(request, subdomain):
    log_request(request, subdomain)
    data = {'raw': '', 'headers': [], 'status_code': 200}
    if not os.path.exists('pages/' + subdomain):
        write_basic_file(subdomain)
    with open('pages/' + subdomain, 'r') as json_file:
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

    if subdomain:
        return subdomain_response(request, subdomain)
    else:
        subdomain = verify_jwt(request.cookies.get('token'))
        return send_from_directory('public', 'index.html')


@app.endpoint('catch_all')
def catch_all(path):
    subdomain = get_subdomain_from_hostname(request.host)
    if subdomain:
        return subdomain_response(request, subdomain)

    subdomain = request.path[1:8 + 1].lower()
    if len(subdomain) == 8 and subdomain.isalnum():
        return subdomain_response(request, subdomain)

    response = send_from_directory('public', path)

    return response


@app.route('/api/get_dns_requests')
def get_dns_requests():
    subdomain = get_subdomain_from_hostname(request.host)
    if subdomain:
        return subdomain_response(request, subdomain)
    else:
        subdomain = verify_jwt(request.cookies.get('token'))
        time = request.args.get('t')
        if subdomain:
            return jsonify(dns_get_subdomain(subdomain, time))
        else:
            return jsonify({'error': 'Unauthorized'}), 401


@app.route('/api/get_http_requests')
def get_http_requests():
    subdomain = get_subdomain_from_hostname(request.host)
    if subdomain:
        return subdomain_response(request, subdomain)
    else:
        subdomain = verify_jwt(request.cookies.get('token'))
        time = request.args.get('t')
        if subdomain:
            return jsonify(http_get_subdomain(subdomain, time))
        else:
            return jsonify({'error': 'Unauthorized'}), 401


@app.route('/api/get_requests')
def get_requests():
    subdomain = get_subdomain_from_hostname(request.host)
    if subdomain:
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
            return jsonify({'error': 'Unauthorized'}), 401


@app.route('/api/get_token', methods=['POST', 'OPTIONS'])
def get_token():
    subdomain = get_subdomain_from_hostname(request.host)
    if subdomain:
        return subdomain_response(request, subdomain)
    else:
        if request.method == 'OPTIONS':
            return 'POST'

        subdomain = get_random_subdomain()
        while users_get_subdomain(subdomain) != None:
            subdomain = get_random_subdomain()
        dns_delete_records(subdomain)

        write_basic_file(subdomain)

        payload = {
            'iat': datetime.datetime.utcnow(),
            'exp': datetime.datetime.utcnow() + datetime.timedelta(days=31),
            'subdomain': subdomain
        }
        token = jwt.encode(payload, JWT_SECRET, algorithm='HS256')
        resp = make_response(token)
        resp.set_cookie('token', token)
        return resp


@app.route('/api/get_server_time')
def get_server_time():
    subdomain = get_subdomain_from_hostname(request.host)
    if subdomain:
        return subdomain_response(request, subdomain)
    else:
        return jsonify({'date': datetime.datetime.utcnow().strftime('%s')})


@app.route('/api/delete_request', methods=['POST'])
def delete_request():
    subdomain = get_subdomain_from_hostname(request.host)
    if subdomain:
        return subdomain_response(request, subdomain)
    else:
        subdomain = verify_jwt(request.cookies.get('token'))
        if subdomain:
            content = request.json
            if content:
                _id = content.get('id')
                rtype = content.get('type')
                delete_request_from_db(_id, subdomain, rtype)
                return jsonify({"rtype": rtype, "_id": _id})
        return jsonify({"error": "Unauthorized"}), 401


@app.route('/api/get_file', methods=['GET'])
def get_file():
    subdomain = get_subdomain_from_hostname(request.host)
    if subdomain:
        return subdomain_response(request, subdomain)
    subdomain = verify_jwt(request.cookies.get('token'))
    if subdomain:
        with open('pages/' + subdomain, 'r') as outfile:
            return outfile.read()
    else:
        return jsonify({"raw": "", "headers": [], "status_code": 200})


@app.route('/api/update_file', methods=['POST'])
def update_file():
    subdomain = get_subdomain_from_hostname(request.host)
    if subdomain:
        return subdomain_response(request, subdomain)
    subdomain = verify_jwt(request.cookies.get('token'))
    if subdomain:
        content = request.json
        status_code = 200
        if 'status_code' in content:
            try:
                try:
                    if len(content['status_code']) > 9:
                        return jsonify({"error": "invalid status_code"}), 401
                    status_code = int(content['status_code'])
                except:
                    pass
            except:
                return jsonify({"error": "invalid status_code"}), 401
        raw = ""
        if 'raw' in content:
            if len(content['raw']) <= 2000000:
                try:
                    base64.b64decode(content['raw'])
                    raw = content['raw']
                except:
                    return jsonify({"error": "invalid response"}), 401
            else:
                return jsonify(
                    {"error": "response should be smaller than 2MB"}), 401
        headers = []
        if 'headers' in content:
            if len(headers) <= 30:
                for header in content['headers']:
                    if 'header' in header and 'value' in header:
                        headers.append({
                            'header': header['header'],
                            'value': header['value']
                        })
            else:
                return jsonify({"error": "maximum of 30 headers"}), 401
            with open('pages/' + subdomain, 'w') as outfile:
                json.dump(
                    {
                        'headers': headers,
                        'raw': raw,
                        'status_code': status_code
                    }, outfile)
        return jsonify({"msg": "Updated response"})
    return jsonify({"error": "Unauthorized"}), 401


@app.route('/api/get_dns_records', methods=['GET'])
def get_dns_records():
    subdomain = get_subdomain_from_hostname(request.host)
    if subdomain:
        return subdomain_response(request, subdomain)
    subdomain = verify_jwt(request.cookies.get('token'))
    if subdomain:
        return jsonify(dns_get_records(subdomain))
    return jsonify({"error": "Unauthorized"}), 401


DNS_RECORDS = ['A', 'AAAA', 'CNAME', 'TXT']


@app.route('/api/update_dns_records', methods=['POST'])
def update_dns_records():
    subdomain = get_subdomain_from_hostname(request.host)
    if subdomain:
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

                domain = domain.lower()
                try:
                    if len(domain) > 63:
                        return jsonify({"error": "Domain too big"}), 401
                        
                    if len(value) > 255:
                        return jsonify({"error": "Value too big"}), 401
                        
                    if type(dtype) is not int:
                        return jsonify({"error": "Invalid type"}), 401
                        
                    if dtype < 0 or dtype >= len(DNS_RECORDS):
                        return jsonify({"error": "Invalid type range"}), 401
                        
                    if not re.search("^[ -~]+$", value):
                        return jsonify({"error": "Invailid regex1"}), 401
                        
                    if not re.match(
                            "^[A-Za-z0-9](?:[A-Za-z0-9\\-_\\.]{0,61}[A-Za-z0-9])?$",
                            domain):
                        return jsonify({"error": "invalid regex2"}), 401
                        
                    domain = domain + '.' + subdomain + '.requestrepo.com.'
                    dtype = DNS_RECORDS[dtype]
                    dns_insert_record(subdomain, domain, dtype, value)
                except Exception as e:
                    return jsonify({"error": "' + str(e) + '"}), 401
                    
            return jsonify({"msg": "Updated records"})
        return jsonify({"error": "Invalid records"}), 401

    return jsonify({"error": "unauthenticated"}), 401


if __name__ == '__main__':
    app.run(host='0.0.0.0', port=21337)
