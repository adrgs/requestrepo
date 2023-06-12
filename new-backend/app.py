from fastapi import FastAPI, HTTPException
from starlette.responses import JSONResponse
from starlette.routing import Route
from starlette import status
from utils import (
    get_subdomain_from_hostname,
    write_basic_file,
    get_random_subdomain,
    get_subdomain_from_path,
)
from aioredis import from_url
from config import config
from pathlib import Path
import base64
from fastapi.responses import FileResponse, Response
import json
import datetime
import uuid
import jwt
from fastapi.websockets import WebSocket, WebSocketDisconnect

app = FastAPI()

redis = None


@app.on_event("startup")
async def startup_event():
    global redis
    redis = await from_url(
        f"redis://{config.redis_host}", encoding="utf-8", decode_responses=True
    )


@app.on_event("shutdown")
async def shutdown_event():
    global redis
    if redis is not None:
        await redis.close()


async def log_request(request, subdomain):
    dic = {}
    headers = dict(request.headers)

    print(request.scope)

    dic["_id"] = str(uuid.uuid4())
    dic["type"] = "http"
    dic["raw"] = base64.b64encode(await request.body()).decode()
    dic["uid"] = subdomain
    dic["ip"] = request.client.host
    dic["port"] = request.client.port
    dic["headers"] = headers
    dic["method"] = request.method
    dic["protocol"] = request.scope["scheme"].upper() + '/' + request.scope["http_version"]
    dic["path"] = request.url.path
    dic["fragment"] = "#" + request.url.fragment if request.url.fragment else ""
    dic["query"] = "?" + request.url.query if request.url.query else ""
    dic["url"] = str(request.url)
    dic["date"] = int(datetime.datetime.now(datetime.timezone.utc).timestamp())

    data = json.dumps(dic)

    await redis.publish(f"pubsub:{subdomain}", data)
    idx = await redis.rpush(f"requests:{subdomain}", data)
    await redis.set(f"request:{dic['_id']}", idx)


def verify_jwt(token):
    try:
        return jwt.decode(token, config.jwt_secret, algorithms=["HS256"])["subdomain"]
    except Exception:
        return None


@app.get("/api/get_file")
async def get_file(token: str):
    subdomain = verify_jwt(token)

    if subdomain is None:
        raise HTTPException(status_code=403, detail="Invalid token")

    subdomain_path = Path(f"pages/") / Path(subdomain).name
    if not subdomain_path.exists():
        write_basic_file(subdomain)

    with open("pages/" + subdomain, "r") as json_file:
        return Response(json_file.read())


@app.post("/api/get_token")
async def get_token():
    subdomain = get_random_subdomain()

    while await redis.exists(f"subdomain:{subdomain}"):
        subdomain = get_random_subdomain()

    await redis.set(f"subdomain:{subdomain}", 1)

    write_basic_file(subdomain)

    payload = {
        "iat": datetime.datetime.utcnow(),
        "exp": datetime.datetime.utcnow() + datetime.timedelta(days=31),
        "subdomain": subdomain,
    }

    token = jwt.encode(payload, config.jwt_secret, algorithm="HS256")

    return JSONResponse({"token": token, "subdomain": subdomain})


@app.websocket("/api/ws")
async def websocket_endpoint(websocket: WebSocket):
    await websocket.accept()

    token = await websocket.receive_text()
    subdomain = verify_jwt(token)

    if subdomain is None:
        await websocket.close(code=status.WS_1008_POLICY_VIOLATION)
        return

    requests = await redis.lrange(f"requests:{subdomain}", 0, -1)
    requests = [request for request in requests if request != "{}"]

    await websocket.send_json({"cmd":"requests", "data": requests})

    pubsub = redis.pubsub()
    await pubsub.subscribe(f"pubsub:{subdomain}")
    async for message in pubsub.listen():
        if message["type"] == "message":
            await websocket.send_json({"cmd":"request", "data": message["data"]})


async def catch_all(request):
    host = request.headers.get("host")
    subdomain = get_subdomain_from_hostname(host) or get_subdomain_from_path(
        request.url.path
    )

    if subdomain is None:
        path = Path(request.url.path)
        path = Path(f"public/") / path.relative_to(path.anchor)
        if not path.exists() or path.is_dir():
            return FileResponse("public/index.html")
        return FileResponse(path)

    subdomain_path = Path(f"pages/") / Path(subdomain).name
    if not subdomain_path.exists():
        write_basic_file(subdomain)

    data = {"raw": "", "headers": {}, "status_code": 200}

    with open("pages/" + subdomain, "r") as json_file:
        try:
            data = json.load(json_file)
        except Exception:
            pass
    try:
        resp = Response(base64.b64decode(data["raw"]))
    except Exception:
        resp = Response(b"")

    resp.headers.update(data["headers"])
    resp.status_code = data["status_code"]

    await log_request(request, subdomain)

    return resp


catch_all_route = Route("/{path:path}", endpoint=catch_all, methods=[])
app.router.routes.append(catch_all_route)
