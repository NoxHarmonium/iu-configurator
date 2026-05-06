FROM python:3.14-slim-trixie

# Non-root user (568:568 matches TrueCharts' home assistant UID/GID)
RUN groupadd -g 568 appgroup && useradd -m -u 568 -g 568 appuser

USER appuser

RUN pip install --no-cache-dir requests websocket-client

COPY bootstrap.py .

CMD ["python", "bootstrap.py"]
