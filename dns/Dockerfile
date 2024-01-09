FROM python:3.11

COPY ./dns/ /app/
COPY ./ip2country/ /app/ip2country/
WORKDIR /app

RUN pip install -r requirements.txt

RUN useradd -ms /bin/bash app
USER app

CMD ["python3", "./ns.py"]