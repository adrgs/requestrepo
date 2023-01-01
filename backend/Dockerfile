FROM python:3
RUN mkdir /app
WORKDIR /app
EXPOSE 21337
COPY /requirements.txt .
RUN pip install -r requirements.txt
COPY ./app.py /app/app.py
COPY ./public /app/public
COPY ./app.ini /app/app.ini
#CMD [ "uwsgi", "--ini", "app.ini" ]
CMD ["python", "app.py"]