FROM alpine:latest

RUN apk add --no-cache python3 && \
    python3 -m ensurepip && \
    rm -r /usr/lib/python*/ensurepip && \
    pip3 install --upgrade pip setuptools && \
    if [ ! -e /usr/bin/pip ]; then ln -s pip3 /usr/bin/pip ; fi && \
    if [[ ! -e /usr/bin/python ]]; then ln -sf /usr/bin/python3 /usr/bin/python; fi && \
    rm -r /root/.cache && rm -rf /tmp/*

WORKDIR /src

COPY ./src/requirements.txt .
RUN ls -lh
RUN pip install --no-cache-dir -r requirements.txt

COPY ./src /src

CMD [ "python3", "./daemon.py" ]
