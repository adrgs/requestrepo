FROM python:3.10

RUN apt-get update && apt-get install -y \
    software-properties-common \
    npm
RUN npm install npm@latest -g && \
    npm install n -g && \
    n latest

COPY ./frontend /tmp/frontend
WORKDIR /tmp/frontend

RUN npm install --force
RUN npm run build

COPY ./new-backend /app
RUN cp -r /tmp/frontend/build/* /app/public/
RUN rm -rf /tmp/frontend

WORKDIR /app
RUN pip install -r requirements.txt
RUN chmod 703 /app/pages

COPY start.sh /app/start.sh
RUN chmod 755 /app/start.sh

COPY privkey.pem /etc/privkey.pem
COPY fullchain.pem /etc/fullchain.pem

RUN chmod 644 /etc/privkey.pem
RUN chmod 644 /etc/fullchain.pem

RUN useradd -ms /bin/bash app
USER app

EXPOSE 80
EXPOSE 443

CMD ["/app/start.sh"]