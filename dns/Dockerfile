FROM python:3.10

COPY ./ /app/
WORKDIR /app

RUN pip install -r requirements.txt

RUN useradd -ms /bin/bash app
USER app

CMD ["python3", "./ns.py"]