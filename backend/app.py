import base64
import datetime
import json
import logging
import re
import uuid
from collections import defaultdict
from contextlib import asynccontextmanager
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, AsyncIterator, Dict, List, Union

import asyncio
import ip2country
import jwt
from config import config
from fastapi import Depends, FastAPI, HTTPException, Request
from fastapi.responses import FileResponse, JSONResponse, Response
from fastapi.websockets import WebSocket, WebSocketDisconnect
from fastapi_utils.tasks import repeat_every
from models import (
    DeleteRequest,
    DnsRecords,
    File,
    HttpRequestLog,
)
from pydantic import BaseModel
from redis import asyncio as aioredis
from starlette import status
from starlette.requests import ClientDisconnect
from starlette.responses import JSONResponse
from starlette.routing import Route
from utils import (
    get_random_subdomain,
    get_subdomain_from_hostname,
    get_subdomain_from_path,
    verify_jwt,
    verify_subdomain,
    write_basic_file,
)
from websockets.exceptions import ConnectionClosed

app = FastAPI(server_header=False)


@dataclass
class SessionManager:
    websocket: WebSocket
    redis: aioredis.Redis
    sessions: set = field(default_factory=set)  # set of subdomains
    pubsub: aioredis.Redis.pubsub = None

    async def add_session(self, token: str) -> bool:
        try:
            subdomain = verify_jwt(token)
            if subdomain is None:
                return False

            # If this subdomain is already in a session, don't add it again
            if subdomain in self.sessions:
                return True

            # Create pubsub if it doesn't exist
            if self.pubsub is None:
                self.pubsub = self.redis.pubsub()

            # Add to sessions first
            self.sessions.add(subdomain)

            # Then subscribe to the channel
            await self.pubsub.subscribe(f"pubsub:{subdomain}")

            # Send historical requests for the new session
            requests = await self.redis.lrange(f"requests:{subdomain}", 0, -1)
            requests = [req for req in requests if req != "{}"]
            await self.websocket.send_json(
                {"cmd": "requests", "data": requests, "subdomain": subdomain}
            )

            return True
        except Exception as e:
            logger.error(f"Error adding session: {e}")
            if subdomain in self.sessions:
                self.sessions.remove(subdomain)
            return False

    async def remove_session(self, token: str) -> None:
        try:
            subdomain = verify_jwt(token)
            if subdomain in self.sessions:
                if self.pubsub:
                    await self.pubsub.unsubscribe(f"pubsub:{subdomain}")
                self.sessions.remove(subdomain)
        except Exception as e:
            logger.error(f"Error removing session: {e}")

    async def remove_all_sessions(self) -> None:
        try:
            if self.pubsub and self.sessions:
                # Unsubscribe from all channels
                channels = [f"pubsub:{subdomain}" for subdomain in self.sessions]
                if channels:
                    await self.pubsub.unsubscribe(*channels)
            self.sessions.clear()
        except Exception as e:
            logger.error(f"Error removing all sessions: {e}")
            # Still clear sessions even if there's an error
            self.sessions.clear()

    def get_subdomain(self, token: str) -> str:
        return verify_jwt(token)

    async def cleanup(self) -> None:
        """Clean up all resources properly"""
        try:
            await self.remove_all_sessions()
            if self.pubsub:
                await self.pubsub.close()
                self.pubsub = None
        except Exception as e:
            logger.error(f"Error in SessionManager cleanup: {e}")


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

    async def close_pool(self):
        await self.pool.disconnect()


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
    await redis_dependency.close_pool()  # Close the entire pool on shutdown


def validation_error(msg: str) -> Response:
    response = JSONResponse({"error": msg})
    response.status_code = 401
    return response


@app.post("/api/update_dns")
async def update_dns(
    records: DnsRecords,
    token: str,
    redis: aioredis.Redis = Depends(redis_dependency.get_redis),
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

    pipeline = await redis.pipeline()

    for record in records.records:
        new_domain = f"{record.domain.lower()}.{subdomain}.{config.server_domain}."
        new_value = record.value
        new_dtype = DNS_RECORDS[record.type]

        key = f"dns:{new_dtype}:{new_domain}"

        new_record = {"domain": new_domain, "type": new_dtype, "value": new_value}
        final_records.append(new_record)

        values[key].append(new_value)

    for key, value in values.items():
        await pipeline.set(key, json.dumps(value), ex=config.redis_ttl)

    # Store all records for this subdomain
    await pipeline.set(
        f"dns:{subdomain}", json.dumps(final_records), ex=config.redis_ttl
    )

    await pipeline.execute()

    return JSONResponse({"msg": "Updated DNS records"})


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
async def get_file(
    token: str, redis: aioredis.Redis = Depends(redis_dependency.get_redis)
) -> Response:
    subdomain = verify_jwt(token)
    if subdomain is None:
        raise HTTPException(status_code=403, detail="Invalid token")

    # Get the file tree first
    tree_data = await redis.get(f"files:{subdomain}")
    if not tree_data:
        await write_basic_file(subdomain, redis)
        tree_data = await redis.get(f"files:{subdomain}")

    tree = json.loads(tree_data)
    return JSONResponse(tree["index.html"])


@app.get("/api/get_request")
async def get_request(
    subdomain: str, id: str, redis: aioredis.Redis = Depends(redis_dependency.get_redis)
) -> Response:
    try:
        uuid.UUID(id)
    except Exception:
        raise HTTPException(status_code=404, detail="Invalid request ID")

    if not verify_subdomain(subdomain):
        raise HTTPException(status_code=404, detail="Invalid subdomain")

    idx = await redis.get(f"request:{subdomain}:{id}")
    if idx is None:
        raise HTTPException(status_code=404, detail="Request not found")

    request = await redis.lindex(f"requests:{subdomain}", idx)
    if request is None:
        raise HTTPException(status_code=404, detail="Request not found")

    return JSONResponse(json.loads(request))


@app.post("/api/delete_request")
async def delete_request(
    req: DeleteRequest,
    token: str,
    redis: aioredis.Redis = Depends(redis_dependency.get_redis),
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
async def update_file(
    file: File, token: str, redis: aioredis.Redis = Depends(redis_dependency.get_redis)
) -> Response:
    subdomain = verify_jwt(token)
    if subdomain is None:
        raise HTTPException(status_code=403, detail="Invalid token")

    if len(file.raw) > config.max_file_size:
        return JSONResponse({"error": "Response too large"})

    # Get existing tree or create new one
    tree_data = await redis.get(f"files:{subdomain}")
    if tree_data:
        tree = json.loads(tree_data)
    else:
        tree = {}

    # Update index.html in the tree
    tree["index.html"] = file.model_dump()

    await redis.set(f"files:{subdomain}", json.dumps(tree), ex=config.redis_ttl)

    return JSONResponse({"msg": "Updated response"})


@app.post("/api/get_token")
async def get_token(
    redis: aioredis.Redis = Depends(redis_dependency.get_redis),
) -> Response:
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
async def old_websocket_endpoint(
    websocket: WebSocket, redis: aioredis.Redis = Depends(redis_dependency.get_redis)
) -> None:
    pubsub = None
    try:
        await websocket.accept()

        token = await websocket.receive_text()
        subdomain = verify_jwt(token)

        if subdomain is None:
            await websocket.send_json({"cmd": "invalid_token", "token": token})
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
    except Exception as e:
        logger.error(f"WebSocket error: {e}")
    finally:
        # Properly clean up resources
        if pubsub:
            try:
                await pubsub.unsubscribe(f"pubsub:{subdomain}")
                await pubsub.close()
            except Exception as e:
                logger.error(f"Error closing pubsub: {e}")


@app.websocket("/api/ws2")
async def websocket_endpoint(
    websocket: WebSocket, redis: aioredis.Redis = Depends(redis_dependency.get_redis)
) -> None:
    session_manager = SessionManager(websocket=websocket, redis=redis)
    try:
        await websocket.accept()
        logger.info("WebSocket connected")

        # Receive initial message
        init_data = await websocket.receive_json()

        # Handle both single token and multiple sessions format
        if "cmd" in init_data and init_data["cmd"] == "register_sessions":
            sessions = init_data.get("sessions", [])
        else:
            # Legacy single token format
            sessions = [
                {
                    "token": init_data.get("token"),
                    "subdomain": init_data.get("subdomain"),
                }
            ]

        # Validate and setup sessions
        valid_sessions = []
        for session in sessions:
            if await session_manager.add_session(session["token"]):
                valid_sessions.append(session)
            else:
                await websocket.send_json(
                    {"cmd": "invalid_token", "token": session["token"]}
                )

        if not valid_sessions:
            await websocket.send_json({"error": "No valid sessions provided"})
            return

        # Send initial data and subscribe to channels for each session
        for session in valid_sessions:
            subdomain = session_manager.get_subdomain(session["token"])
            if subdomain:
                # Send historical requests
                requests = await redis.lrange(f"requests:{subdomain}", 0, -1)
                requests = [req for req in requests if req != "{}"]
                await websocket.send_json(
                    {"cmd": "requests", "data": requests, "subdomain": subdomain}
                )

        # Main message loop
        while True:
            try:
                # Handle WebSocket messages (use wait_for to avoid blocking)
                try:
                    ws_message = await asyncio.wait_for(
                        websocket.receive_json(), timeout=0.1
                    )
                    logger.debug(f"Received message: {ws_message}")

                    if ws_message.get("cmd") == "update_tokens":
                        new_tokens = ws_message.get("tokens", [])

                        # Remove old tokens
                        await session_manager.remove_all_sessions()

                        # Add new tokens
                        for token in new_tokens:
                            if not await session_manager.add_session(token):
                                await websocket.send_json(
                                    {"cmd": "invalid_token", "token": token}
                                )
                    elif ws_message.get("cmd") == "ping":
                        await websocket.send_json({"cmd": "pong"})
                except asyncio.TimeoutError:
                    pass

                # Handle pub/sub messages
                if session_manager.pubsub:
                    message = await session_manager.pubsub.get_message(timeout=0.1)
                    if message and message["type"] == "message":
                        channel = message["channel"]
                        subdomain = (
                            channel.decode().split(":")[1]
                            if isinstance(channel, bytes)
                            else channel.split(":")[1]
                        )
                        try:
                            await websocket.send_json(
                                {
                                    "cmd": "request",
                                    "data": message["data"],
                                    "subdomain": subdomain,
                                }
                            )
                        except Exception as e:
                            logger.error(
                                f"Error sending pubsub message for {subdomain}: {e}"
                            )

                # Small delay to prevent CPU spinning
                await asyncio.sleep(0.01)

            except WebSocketDisconnect:
                logger.info("WebSocket disconnected")
                break
            except ConnectionClosed:
                logger.info("Connection closed")
                break
            except Exception as e:
                logger.error(f"WebSocket error: {e}")
                break

    except Exception as e:
        logger.error(f"WebSocket error: {e}")
        raise
    finally:
        logger.info("WebSocket connection closed")
        # Use the new cleanup method to ensure all resources are properly closed
        await session_manager.cleanup()


@app.get("/api/files")
async def get_files(
    token: str, redis: aioredis.Redis = Depends(redis_dependency.get_redis)
) -> Response:
    subdomain = verify_jwt(token)
    if subdomain is None:
        raise HTTPException(status_code=403, detail="Invalid token")

    # Get file tree from Redis
    tree_data = await redis.get(f"files:{subdomain}")
    if not tree_data:
        await write_basic_file(subdomain, redis)
        tree_data = await redis.get(f"files:{subdomain}")

    return JSONResponse(json.loads(tree_data))


@app.post("/api/files")
async def update_files(
    tree: Dict[str, Any],
    token: str,
    redis: aioredis.Redis = Depends(redis_dependency.get_redis),
) -> Response:
    subdomain = verify_jwt(token)
    if subdomain is None:
        raise HTTPException(status_code=403, detail="Invalid token")

    # Validate the tree structure
    def validate_tree(tree_dict, path=""):
        for key, value in tree_dict.items():
            current_path = f"{path}{key}"
            if isinstance(value, dict):
                if key.endswith("/"):
                    validate_tree(value, current_path)
                else:
                    if not all(k in value for k in ["raw", "headers", "status_code"]):
                        raise ValueError(f"Invalid file structure for {current_path}")
                    if type(value["raw"]) != str:
                        raise ValueError(
                            f"Invalid raw file structure for {current_path}"
                        )
                    if type(value["headers"]) != list:
                        raise ValueError(
                            f"Invalid headers file structure for {current_path}"
                        )
                    if type(value["status_code"]) != int:
                        raise ValueError(
                            f"Invalid status_code file structure for {current_path}"
                        )
                    if len(value["raw"]) > config.max_file_size:
                        raise ValueError(f"File too large: {current_path}")

    try:
        validate_tree(tree)
    except ValueError as e:
        return JSONResponse({"error": str(e)})

    # Ensure index.html exists
    if "index.html" not in tree:
        return JSONResponse({"error": "index.html cannot be deleted"})

    # Store the file tree
    await redis.set(f"files:{subdomain}", json.dumps(tree), ex=config.redis_ttl)

    return JSONResponse({"msg": "Updated files"})


@app.api_route("/{path:path}")
async def catch_all(
    request: Request, redis: aioredis.Redis = Depends(redis_dependency.get_redis)
) -> Response:
    host = request.headers.get("host") or config.server_domain
    subdomain = get_subdomain_from_hostname(host) or get_subdomain_from_path(
        request.url.path
    )

    if subdomain is None:
        path = Path(request.url.path)
        public = Path("public/").resolve()
        path = (public / path.relative_to(path.anchor)).resolve()
        if not path.exists() or path.is_dir() or not path.is_relative_to(public):
            if Path("public/index.html").exists():
                response = FileResponse("public/index.html")
            else:
                response = JSONResponse({"error": "Not found"}, status_code=404)
        else:
            response = FileResponse(path)
        response.headers["Access-Control-Allow-Origin"] = "*"
        return response

    data = await redis.get(f"files:{subdomain}")
    if not data:
        await write_basic_file(subdomain, redis)
        data = await redis.get(f"files:{subdomain}")

    try:
        data = json.loads(data)
    except Exception:
        data = {"index.html": {"raw": "", "headers": [], "status_code": 200}}

    # Normalize path by removing duplicate slashes and trailing slash
    path = re.sub("/+", "/", request.url.path.strip("/"))

    # Split path into components
    path_parts = path.split("/")
    current_data = data
    file_data = None

    # Traverse the path
    for part in path_parts:
        if not part:
            continue

        if part in current_data:
            if isinstance(current_data[part], dict):
                file_data = current_data[part]
                break
        elif part + "/" in current_data:
            # It's a directory
            current_data = current_data[part + "/"]
            if "index.html" in current_data:
                file_data = current_data["index.html"]
        else:
            break

    if not file_data:
        file_data = data["index.html"]

    try:
        resp = Response(base64.b64decode(file_data["raw"]))
    except Exception:
        resp = Response(b"")

    headers_obj = {}

    for header in file_data["headers"]:
        key = header["header"]
        value = header["value"]
        headers_obj[key] = value

    resp.headers.update(headers_obj)
    resp.status_code = file_data["status_code"]

    await log_request(request, subdomain, redis)

    return resp


class AlwaysTrueSet(set):
    def __contains__(self, item):
        return True


list(filter(lambda r: r.name == "catch_all", app.routes))[0].methods = AlwaysTrueSet()


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
    await redis.expire(f"requests:{subdomain}", config.redis_ttl)
    await redis.set(f"request:{subdomain}:{request_log['_id']}", idx)
