from fastapi import FastAPI, HTTPException, Depends, Request
from contextlib import asynccontextmanager
from starlette.responses import JSONResponse
from starlette.requests import ClientDisconnect
from starlette.routing import Route
from starlette import status
from backend.utils import (
    get_subdomain_from_hostname,
    write_basic_file,
    get_random_subdomain,
    get_subdomain_from_path,
    verify_jwt,
)
from redis import asyncio as aioredis
from collections import defaultdict
from backend.config import config
from pathlib import Path
from typing import AsyncIterator
from fastapi.responses import FileResponse, Response
from fastapi.websockets import WebSocket, WebSocketDisconnect
from websockets.exceptions import ConnectionClosed
from backend.models import HttpRequestLog, File, DeleteRequest, DnsRecords, RequestRepoResponse
import base64
import json
import datetime
import uuid
import jwt
import re
import ip2country
from fastapi_utils.tasks import repeat_every
import logging
import aiofiles

app = FastAPI(server_header=False)


class RedisDependency:
    def __init__(self):
        self.pool = aioredis.ConnectionPool.from_url(
            f"redis://{config.redis_host}",
            encoding="utf-8",
            decode_responses=True,
            max_connections=1024 * 1024,
        )

    async def get_redis(self) -> aioredis.Redis:
        return aioredis.Redis(connection_pool=self.pool)


redis_dependency = RedisDependency()
logger = logging.getLogger("uvicorn.error")


@repeat_every(seconds=6 * 60 * 60)  # 6 hours
async def renew_certificate() -> None:
    redis = await redis_dependency.get_redis()
    lock = redis.lock("renewer_lock", timeout=3600)  # Lock timeout is 1 hour
    if not await lock.acquire(blocking=False):
        return

    logger.info("Acquired lock for renewer")
    try:
        """
        if not is_certificate_expiring_or_untrusted(
            "/app/cert/fullchain.pem", config.server_domain
        ):
            logger.info("Certificate is valid")
            return
        """

        logger.info("Renewing certificate")

        async def update_dns(domain, tokens):
            key = f"dns:TXT:{domain}."
            await redis.set(key, json.dumps(tokens))
            logger.info(f"Updated DNS for {domain} with tokens {tokens}")

        # await get_certificate(config.server_domain, "/app/cert/", update_dns)
    except Exception as e:
        logger.error(f"Error in renewer: {e}")
    finally:
        await lock.release()
        logger.info("Released lock for renewer")


@asynccontextmanager
async def lifespan(app: FastAPI) -> AsyncIterator[None]:
    redis = await redis_dependency.get_redis()
    await renew_certificate()

    yield

    await redis.close()


def validation_error(msg: str) -> Response:
    response = JSONResponse({"error": msg})
    response.status_code = 401
    return response


@app.post("/api/update_dns")
async def update_dns(
    records: DnsRecords, token: str, redis: aioredis.Redis = Depends(redis_dependency.get_redis)
) -> Response:
    DNS_RECORDS = ["A", "AAAA", "CNAME", "TXT"]

    subdomain = verify_jwt(token)
    if subdomain is None:
        raise HTTPException(status_code=403, detail="Invalid token")

    # Validate entries
    for record in records.records:
        domain = record.domain.lower()
        value = record.value
        dtype = record.type

        if not domain or not value:
            continue

        if len(domain) > 63:
            return validation_error(f"Domain name '{domain}' too long")

        if len(value) > 255:
            return validation_error(f"Value '{value}' too long")

        if dtype < 0 or dtype >= len(DNS_RECORDS):
            return validation_error(f"Invalid type for domian {domain}")

        if not re.search("^[ -~]+$", value) and dtype != 3:
            return validation_error(f"Invalid characters in value '{value}'")

        if not re.match(
            "^[A-Za-z0-9](?:[A-Za-z0-9\\-_\\.]{0,61}[A-Za-z0-9])?$", domain
        ):
            return validation_error(f"Invalid characters in domain '{domain}'")

    # Delete old entries
    old_records = await redis.get(f"dns:{subdomain}")
    if old_records:
        old_records = json.loads(old_records)
        for old_record in old_records:
            await redis.delete(f"dns:{old_record['type']}:{old_record['domain']}")

    # Update if all entries are valid
    final_records = []

    values = defaultdict(list)

    for record in records.records:
        new_domain = f"{record.domain.lower()}.{subdomain}.{config.server_domain}."
        new_value = record.value
        new_dtype = DNS_RECORDS[record.type]

        key = f"dns:{new_dtype}:{new_domain}"

        new_record = {"domain": new_domain, "type": new_dtype, "value": new_value}
        final_records.append(new_record)

        values[key].append(new_value)

    for key, value in values.items():
        await redis.set(key, json.dumps(value), ex=config.redis_ttl)

    await redis.set(f"dns:{subdomain}", json.dumps(final_records), ex=config.redis_ttl)

    return JSONResponse({"msg": "Updated records"})


@app.get("/api/get_dns")
async def get_dns(
    token: str, redis: aioredis.Redis = Depends(redis_dependency.get_redis)
) -> Response:
    subdomain = verify_jwt(token)
    if subdomain is None:
        raise HTTPException(status_code=403, detail="Invalid token")

    records = await redis.get(f"dns:{subdomain}")

    if records is None:
        return JSONResponse([])

    return JSONResponse(json.loads(records))


@app.get("/api/get_file")
async def get_file(token: str, redis: aioredis.Redis = Depends(redis_dependency.get_redis)) -> Response:
    subdomain = verify_jwt(token)
    if subdomain is None:
        raise HTTPException(status_code=403, detail="Invalid token")

    data = await redis.get(f"page:{subdomain}")
    if not data:
        await write_basic_file(subdomain, redis)
        data = await redis.get(f"page:{subdomain}")

    return Response(data)


@app.post("/api/delete_request")
async def delete_request(
    req: DeleteRequest, token: str, redis: aioredis.Redis = Depends(redis_dependency.get_redis)
) -> Response:
    subdomain = verify_jwt(token)
    if subdomain is None:
        raise HTTPException(status_code=403, detail="Invalid token")

    id = req.id

    idx = await redis.get(f"request:{subdomain}:{id}")

    if idx is not None:
        await redis.lset(f"requests:{subdomain}", idx, "{}")
        await redis.delete(f"request:{subdomain}:{id}")

    return JSONResponse({"msg": "Deleted request"})


@app.post("/api/delete_all")
async def delete_all(
    token: str, redis: aioredis.Redis = Depends(redis_dependency.get_redis)
) -> Response:
    subdomain = verify_jwt(token)
    if subdomain is None:
        raise HTTPException(status_code=403, detail="Invalid token")

    requests = await redis.lrange(f"requests:{subdomain}", 0, -1)
    requests = [request for request in requests if request != "{}"]

    ids = [json.loads(request)["_id"] for request in requests]

    await redis.delete(f"requests:{subdomain}")

    for id in ids:
        await redis.delete(f"request:{subdomain}:{id}")

    return JSONResponse({"msg": "Deleted all requests"})


@app.post("/api/update_file")
async def update_file(file: File, token: str, redis: aioredis.Redis = Depends(redis_dependency.get_redis)) -> Response:
    subdomain = verify_jwt(token)
    if subdomain is None:
        raise HTTPException(status_code=403, detail="Invalid token")

    if len(file.raw) > config.max_file_size:
        return JSONResponse({"error": "Response too large"})

    # Store file data in Redis instead of filesystem
    await redis.set(f"page:{subdomain}", file.model_dump_json(), ex=config.redis_ttl)

    return JSONResponse({"msg": "Updated response"})


@app.post("/api/get_token")
async def get_token(redis: aioredis.Redis = Depends(redis_dependency.get_redis)) -> Response:
    subdomain = get_random_subdomain()

    while await redis.exists(f"subdomain:{subdomain}"):
        subdomain = get_random_subdomain()

    await redis.set(f"subdomain:{subdomain}", 1, ex=config.redis_ttl)

    await write_basic_file(subdomain, redis)

    payload = {
        "iat": datetime.datetime.utcnow(),
        "exp": datetime.datetime.utcnow() + datetime.timedelta(days=31),
        "subdomain": subdomain,
    }

    token = jwt.encode(payload, config.jwt_secret, algorithm="HS256")

    return JSONResponse({"token": token, "subdomain": subdomain})


@app.websocket("/api/ws")
async def websocket_endpoint(
    websocket: WebSocket, redis: aioredis.Redis = Depends(redis_dependency.get_redis)
) -> None:
    try:
        await websocket.accept()

        token = await websocket.receive_text()
        subdomain = verify_jwt(token)

        if subdomain is None:
            await websocket.send_json({"cmd": "invalid_token"})
            await websocket.close(code=status.WS_1008_POLICY_VIOLATION)
            return

        requests = await redis.lrange(f"requests:{subdomain}", 0, -1)
        requests = [request for request in requests if request != "{}"]

        await websocket.send_json({"cmd": "requests", "data": requests})

        pubsub = redis.pubsub()
        await pubsub.subscribe(f"pubsub:{subdomain}")
        async for message in pubsub.listen():
            if message["type"] == "message":
                await websocket.send_json({"cmd": "request", "data": message["data"]})

    except (WebSocketDisconnect, ConnectionClosed):
        # Handle the disconnection gracefully
        pass
    finally:
        # Perform any necessary cleanup
        if 'pubsub' in locals():
            await pubsub.unsubscribe(f"pubsub:{subdomain}")



async def catch_all(request: Request) -> Response:
    host = request.headers.get("host") or config.server_domain
    subdomain = get_subdomain_from_hostname(host) or get_subdomain_from_path(
        request.url.path
    )

    if subdomain is None:
        path = Path(request.url.path)
        public = Path("public/").resolve()
        path = (public / path.relative_to(path.anchor)).resolve()
        if not path.exists() or path.is_dir() or not path.is_relative_to(public):
            response = FileResponse("public/index.html")
        else:
            response = FileResponse(path)
        response.headers["Access-Control-Allow-Origin"] = "*"
        return response

    redis: aioredis.Redis = await redis_dependency.get_redis()

    data = await redis.get(f"page:{subdomain}")
    if not data:
        await write_basic_file(subdomain, redis)
        data = await redis.get(f"page:{subdomain}")

    try:
        data = json.loads(data)
    except Exception:
        data = {"raw": "", "headers": [], "status_code": 200}

    try:
        resp = Response(base64.b64decode(data["raw"]))
    except Exception:
        resp = Response(b"")

    headers_obj = {}

    for header in data["headers"]:
        key = header["header"]
        value = header["value"]
        headers_obj[key] = value

    resp.headers.update(headers_obj)
    resp.status_code = data["status_code"]

    await log_request(request, subdomain, redis)

    return resp


catch_all_route = Route("/{path:path}", endpoint=catch_all, methods=[])
app.router.routes.append(catch_all_route)


async def log_request(request: Request, subdomain: str, redis: aioredis.Redis) -> None:
    ip, port = (
        (request.client.host, request.client.port)
        if request.client
        else ("127.0.0.1", 1337)
    )

    headers = dict(request.headers)

    body = b""
    try:
        async for chunk in request.stream():
            body += chunk
            if len(body) > config.max_request_size:
                break
    except ClientDisconnect:
        pass

    request_log: HttpRequestLog = HttpRequestLog(
        _id=str(uuid.uuid4()),
        type="http",
        raw=base64.b64encode(body).decode(),
        uid=subdomain,
        ip=ip,
        port=port,
        headers=headers,
        method=request.method,
        protocol=request.scope["scheme"].upper() + "/" + request.scope["http_version"],
        path=request.url.path,
        fragment="#" + request.url.fragment if request.url.fragment else "",
        query="?" + request.url.query if request.url.query else "",
        url=str(request.url),
        date=int(datetime.datetime.now(datetime.timezone.utc).timestamp()),
    )

    ip_country = ip2country.ip_to_country(ip)
    if ip_country is not None:
        request_log["country"] = ip_country

    data = json.dumps(request_log)

    await redis.publish(f"pubsub:{subdomain}", data)
    idx = await redis.rpush(f"requests:{subdomain}", data) - 1
    await redis.set(
        f"request:{subdomain}:{request_log['_id']}", 
        idx,
        ex=config.redis_ttl
    )
