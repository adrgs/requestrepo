from app import app
from uvicorn.workers import UvicornWorker


class ServerlessUvicornWorker(UvicornWorker):
  def __init__(self, *args, **kwargs):
    self.CONFIG_KWARGS["server_header"] = False
    super().__init__(*args, **kwargs)


if __name__ == "__main__":
  app.run()
