import random
import os

SUBDOMAIN_ALPHABET = "0123456789abcdefghijklmnopqrstuvwxyz"
SUBDOMAIN_LENGTH = int(os.environ.get("SUBDOMAIN_LENGTH", 8))


def get_random_subdomain() -> str:
    return "".join(random.choices(SUBDOMAIN_ALPHABET, k=SUBDOMAIN_LENGTH))
