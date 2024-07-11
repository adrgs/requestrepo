# Stage 1: Build frontend
FROM node:20 AS frontend-build

# Cache npm install
WORKDIR /frontend
COPY ./frontend/package.json ./
RUN npm install

COPY ./frontend /frontend

# Set environment variable for Vite
ARG DOMAIN
ENV VITE_DOMAIN=${DOMAIN}

RUN npm run build

FROM python:3.10

COPY ./backend/requirements.txt /app/requirements.txt

WORKDIR /app
RUN pip install -r requirements.txt

COPY ./backend /app
COPY ./ip2country /app/ip2country
# Copy built frontend assets from the first stage
COPY --from=frontend-build /frontend/dist /app/public

RUN chmod 777 /app/pages

COPY start.sh /app/start.sh
RUN chmod 755 /app/start.sh

RUN useradd -ms /bin/bash app

RUN chmod 777 /app/cert/fullchain.pem
RUN chmod 777 /app/cert/privkey.pem

USER app

EXPOSE 80
EXPOSE 443

CMD ["/app/start.sh"]
