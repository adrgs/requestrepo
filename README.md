# requestrepo.com

<img src="https://svgshare.com/i/11Hr.svg" width="420">

Analyze HTTP and DNS requests and create custom DNS records for your subdomain.

![requestrepo demo](https://i.imgur.com/pzn8O18.png)

## Getting Started

These instructions will get you a copy of the project up and running on your local machine for testing purposes.

```sh
git clone https://github.com/adrgs/requestrepo.git
cd requestrepo
cp .env.example .env
# modify .env as needed
docker-compose up --build
```

You can access your instance on localhost HTTP port 80, HTTPS port 443, DNS on port 53 and the Python app directly on port 21337.

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

## Built With

- [React](https://reactjs.org/) - JavaScript library for building user interfaces
- [FastAPI](https://fastapi.tiangolo.com/lo/) - FastAPI framework, high performance, easy to learn, fast to code, ready for production

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on our code of conduct, and the process for submitting pull requests.
