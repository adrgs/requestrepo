from app import app  # noqa: F401
from uvicorn.workers import UvicornWorker


class ServerlessUvicornWorker(UvicornWorker):
    def __init__(self, *args, **kwargs):
        self.CONFIG_KWARGS["server_header"] = False
        super().__init__(*args, **kwargs)
