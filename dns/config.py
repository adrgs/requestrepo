import os


class Config:
  redis_host: str = os.environ.get("REDIS_HOST", "localhost")
  server_ip: str = os.environ.get("SERVER_IP", "127.0.0.1")
  server_domain: str = os.environ.get("DOMAIN", "requestrepo.com").lower()
  include_server_domain: bool = os.environ.get("INCLUDE_SERVER_DOMAIN", "false").lower() == "true"
  subdomain_length: int = int(os.environ.get("SUBDOMAIN_LENGTH", 8))
  subdomain_alphabet: str = os.environ.get("SUBDOMAIN_ALPHABET", "0123456789abcdefghijklmnopqrstuvwxyz")
  subdomain_alphabet_set: set = set(subdomain_alphabet)


config = Config()
