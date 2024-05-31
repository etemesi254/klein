import datetime

import requests
from fastapi import FastAPI, Response

app = FastAPI()

import codecs

NASA = codecs.decode("9PNJiWSUGzpFtreNtHWzyT8w0ute06KVcUDRKpt6", "rot13")


@app.head("/heartbeat")
async def heartbeat():
    return {"status": "alive"}


@app.get("/neo")
async def get_neo(start_date: datetime.datetime, end_date: datetime.datetime, response: Response):
    start_date_formatted = start_date.date()
    params = {
        "start_date": start_date.date(),
        "end_date": end_date.date(),
        "api_key": NASA
    }
    web_resp = requests.get("https://api.nasa.gov/neo/rest/v1/feed", params=params)

    response.status_code = web_resp.status_code
    response.body = web_resp.content

    # response.headers = web_resp.headers
    return response


@app.get("/")
async def root():
    return {"message": "Hello World"}


@app.get("/hello/{name}")
async def say_hello(name: str):
    return {"message": f"Hello {name}"}
