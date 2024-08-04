# requestrepo.com

<img src="https://svgshare.com/i/11Hr.svg" width="420">

Analyze HTTP and DNS requests and create custom DNS records for your subdomain.

## Getting Started

Quick-start using git and docker compose:

```sh
git clone https://github.com/adrgs/requestrepo.git
cd requestrepo
cp .env.example .env # modify .env as needed
docker compose up --build
```

**Don't forget to edit .env**

This will setup a production-ready environment with the following services:
 - HTTP/S server on ports 80 and 443
 - DNS server on port 53


## Setting up DNS nameservers

In order for DNS logging to work, the public IP of the DNS service must be set up as the authoritative nameserver for the domain.

If the domain registrar does not allow setting up IPs directly as nameservers, a workaround is to use a service like [traefik.me](https://traefik.me/)

## Enabling ip2country feature

To enable the ip2country feature, you need to download the free IP to Country Lite database from [db-ip](https://db-ip.com/db/download/ip-to-country-lite).

The csv file must be placed in `ip2country/vendor/dbip-country-lite.csv.gz`

## Development

For development, it is recommended to use the Makefile to start the services for the best developer experience.

### Starting the Services

```sh
# Start the backend service
make start-backend

# Start the frontend service
make start-frontend
```

### Starting the DNS Server

The DNS server needs to be started manually:

```sh
# Start the DNS server
cd dns; python ns.py
```

## Interface

![requestrepo demo](https://i.imgur.com/pzn8O18.png)

## Hall of Fame

Thank you for reporting a security issue in requestrepo:

- [debsec](https://x.com/deb_security) - LFI via improper path handling
- [JaGoTu](https://infosec.exchange/@jagotu) - DoS via unrestricted file upload
- [m0z](https://x.com/LooseSecurity) - LFI via session subdomain

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on our code of conduct, and the process for submitting pull requests.
