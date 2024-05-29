FROM python:3.10

RUN apt-get update && apt-get install -y \
    software-properties-common \
    npm
RUN npm install npm@9.2.0 -g && \
    npm install n -g && \
    n latest

ARG DOMAIN
ENV DOMAIN=${DOMAIN}

COPY ./frontend /tmp/frontend
WORKDIR /tmp/frontend

RUN find . -type f -exec sed -i "s/requestrepo\.com/${DOMAIN}/g" {} +

RUN npm install --force
RUN npm run build

COPY ./backend/requirements.txt /app/requirements.txt

WORKDIR /app
RUN pip install -r requirements.txt

COPY ./backend /app
COPY ./ip2country /app/ip2country
RUN cp -r /tmp/frontend/build/* /app/public/
RUN rm -rf /tmp/frontend

RUN chmod 777 /app/pages

COPY start.sh /app/start.sh
RUN chmod 755 /app/start.sh

RUN useradd -ms /bin/bash app
USER app

EXPOSE 80
EXPOSE 443

CMD ["/app/start.sh"]
