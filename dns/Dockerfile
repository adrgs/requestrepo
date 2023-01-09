FROM python:3

COPY ./requirements.txt /app/requirements.txt
COPY ./ns.py /app/ns.py
COPY ./mongolog.py /app/mongolog.py
WORKDIR /app

RUN pip install -r requirements.txt

RUN useradd -ms /bin/bash app
USER app

CMD ["python3", "./ns.py"]