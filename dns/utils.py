from config import config
from typing import Union


def get_subdomain(domain: str) -> Union[str, None]:
    domain = domain.lower()

    rindex_domain = domain.rfind("." + config.server_domain)

    if rindex_domain == -1:
        return None

    rindex_dot = domain.rfind(".", 0, rindex_domain - 1)

    if rindex_dot == -1:
        subdomain = domain[:rindex_domain]
    else:
        subdomain = domain[rindex_dot + 1 : rindex_domain]

    if len(subdomain) != config.subdomain_length:
        return None

    if not subdomain.isalnum():
        return None

    return subdomain
