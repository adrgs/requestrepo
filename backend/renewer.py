from cryptography.x509 import DNSName, load_pem_x509_certificates
from cryptography.x509.verification import PolicyBuilder, Store, VerificationError

import certifi

from datetime import datetime, timedelta

import simple_acme_dns


def get_certificate(subject: str, cert_path: str, update_dns) -> str:
  client = simple_acme_dns.ACMEClient(
      domains=[subject, "*." + subject],
      email="user@" + subject,
      directory="https://acme-staging-v02.api.letsencrypt.org/directory",
      nameservers=["8.8.8.8", "1.1.1.1"],    # Set the nameservers to query when checking DNS propagation
      new_account=True,    # Register a new ACME account upon creation of our object
      generate_csr=True    # Generate a new private key and CSR upon creation of our object
  )

  for domain, tokens in client.request_verification_tokens().items():
    print(f"{ domain } -> {tokens}")


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
    return False

  if peer.not_valid_after < datetime.now() + timedelta(days=14):
    return False

  return True