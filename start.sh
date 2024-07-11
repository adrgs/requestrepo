#!/bin/sh

# Start Gunicorn on port 80 (plain HTTP)
gunicorn -w 8 -k wsgi.ServerlessUvicornWorker --bind 0.0.0.0:80 wsgi:app &

# Start Gunicorn on port 443 with SSL
gunicorn -w 8 -k wsgi.ServerlessUvicornWorker --bind 0.0.0.0:443 --certfile /app/cert/fullchain.pem --keyfile /app/cert/privkey.pem wsgi:app