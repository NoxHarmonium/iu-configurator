FROM python:3.13-alpine

RUN pip install --no-cache-dir requests websocket-client

COPY bootstrap.py .

CMD ["python", "bootstrap.py"]
