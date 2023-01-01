FROM python:3.6

COPY ./requirements.txt /app/requirements.txt
COPY ./ns.py /app/ns.py
COPY ./mongolog.py /app/mongolog.py
WORKDIR /app

RUN pip install -r requirements.txt

CMD ["./ns.py"]