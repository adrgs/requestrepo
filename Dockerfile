FROM python:3

EXPOSE 21337

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

COPY ./backend /app
RUN cp -r /tmp/frontend/build/* /app/public/
RUN rm -rf /tmp/frontend

WORKDIR /app
RUN pip install -r requirements.txt
RUN chmod 703 /app/pages

RUN useradd -ms /bin/bash app
USER app

CMD ["uwsgi", "--socket", "0.0.0.0:21337", "--protocol=http", "-w", "wsgi:app"]