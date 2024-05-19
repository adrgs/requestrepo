# requestrepo.com

<img src="https://svgshare.com/i/11Hr.svg" width="420">

Analyze HTTP and DNS requests and create custom DNS records for your subdomain.

## Getting Started

These instructions will get you a copy of the project up and running on your local machine for testing purposes.

```sh
git clone https://github.com/adrgs/requestrepo.git
cd requestrepo
cp .env.example .env # modify .env as needed
docker-compose up --build
```

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

For development, it is recommended to start each service individually for the best developer experience

```sh
# start the redis instance
docker run -d --name my-redis -p 6379:6379 redis

# start the backend service
cd backend; uvicorn app:app --port 21337 --no-server-header

# start the frontend service
cd frontend; npm run start

# start the dns server
cd dns; python ns.py
```

## Interface

![requestrepo demo](https://i.imgur.com/pzn8O18.png)

## Built With

- [React](https://reactjs.org/) - JavaScript library for building user interfaces
- [FastAPI](https://fastapi.tiangolo.com/lo/) - FastAPI framework, high performance, easy to learn, fast to code, ready for production

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on our code of conduct, and the process for submitting pull requests.
