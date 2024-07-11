from cryptography.x509 import DNSName, load_pem_x509_certificates
from cryptography.x509.verification import PolicyBuilder, Store, VerificationError

import certifi

from datetime import datetime, timedelta

import simple_acme_dns


async def get_certificate(subject: str, cert_path: str, update_dns) -> str:
    client = simple_acme_dns.ACMEClient(
        domains=[subject, "*." + subject],
        email="user@" + subject,
        directory="https://acme-v02.api.letsencrypt.org/directory",
        nameservers=[
            "8.8.8.8",
            "1.1.1.1",
        ],  # Set the nameservers to query when checking DNS propagation
        new_account=True,  # Register a new ACME account upon creation of our object
        generate_csr=True,  # Generate a new private key and CSR upon creation of our object
    )

    for domain, tokens in client.request_verification_tokens().items():
        await update_dns(domain, tokens)

    if client.check_dns_propagation(timeout=1200):
        client.request_certificate()
        with open(cert_path + "fullchain.pem", "wb") as f:
            f.write(client.certificate)
        with open(cert_path + "privkey.pem", "wb") as f:
            f.write(client.private_key)
    else:
        client.deactivate_account()


def is_certificate_expiring_or_untrusted(cert_path: str, subject: str) -> bool:
    with open(certifi.where(), "rb") as pems:
        store = Store(load_pem_x509_certificates(pems.read()))

    builder = PolicyBuilder().store(store)
    builder = builder.time(datetime.now())

    verifier = builder.build_server_verifier(DNSName(subject))

    with open(cert_path, "rb") as f:
        certs = load_pem_x509_certificates(f.read())
        peer = certs[0]
        untrusted_intermediates = certs[1:]

    try:
        verifier.verify(peer, untrusted_intermediates)
    except VerificationError:
        return True

    if peer.not_valid_after < datetime.now() + timedelta(days=14):
        return True

    return False
