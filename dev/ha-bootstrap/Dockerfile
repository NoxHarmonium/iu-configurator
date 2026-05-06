FROM python:3.14-slim-trixie

# Non-root user (defaults to 568:568 for the TrueCharts' version of home assistant, override with --build-arg)
ARG UID=568
ARG GID=568

RUN groupadd -g ${GID} appgroup && useradd -m -u ${UID} -g ${GID} appuser

USER appuser

RUN pip install --no-cache-dir requests websocket-client

COPY bootstrap.py .

CMD ["python", "bootstrap.py"]
