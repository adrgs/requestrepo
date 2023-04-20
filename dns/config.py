import os
import urllib.parse


class Config:
    mongodb_database: str = urllib.parse.quote_plus(
        os.environ.get("MONGODB_DATABASE", "requestrepo")
    )
    mongodb_username: str = urllib.parse.quote_plus(
        os.environ.get("MONGODB_USERNAME", "requestrepouser")
    )
    mongodb_password: str = urllib.parse.quote_plus(
        os.environ.get("MONGODB_PASSWORD", "changethis")
    )
    mongodb_hostname: str = urllib.parse.quote_plus(
        os.environ.get("MONGODB_HOSTNAME", "127.0.0.1")
    )
    server_ip: str = os.environ.get("SERVER_IP", "127.0.0.1")


config = Config()
