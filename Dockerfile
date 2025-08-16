FROM python:slim-trixie
WORKDIR /src
COPY ./requirements.txt ./
RUN pip install --no-cache-dir -r requirements.txt
COPY ./app /src/app
CMD [ "python3", "app/__init__.py" ]
