# requestrepo.com


<img src="https://svgshare.com/i/pSP.svg" width="420">

Analyze HTTP and DNS requests and create custom DNS records for your subdomain.

![requestrepo demo](https://i.imgur.com/yiap11c.png)


## Getting Started

These instructions will get you a copy of the project up and running on your local machine testing purposes.

```
git clone git@github.com:adrgs/requestrepo.git
cd requestrepo
docker-compose up --build
```

You can access your instance on localhost HTTP port 80, HTTPS port 443, DNS on port 53 and the Python app directly on port 21337.

## Development

For development, it is recommended to start each service individually for the best developer experience

```
# start the mongodb instance
cd backend; docker-compose up --build

# start the backend service
cd backend; python app.py

# start the frontend service
cd frontend; npm run start

# start the dns server
cd dns; python ns.py
```

## Built With

* [React](https://reactjs.org/) - JavaScript library for building user interfaces
* [Flask](https://flask.palletsprojects.com/) - A micro web framework for Python

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on our code of conduct, and the process for submitting pull requests.
