import json
import websocket
import requests

def get_token():
    r = requests.post("https://api.kucoin.com/api/v1/bullet-public")
    return json.loads(r.text)["data"]["token"], json.loads(r.text)["data"]["instanceServers"][0]["endpoint"]

token, endpoint = get_token()

c = websocket.create_connection(f"{endpoint}?token={token}")

welcome = json.loads(c.recv())
assert welcome["type"] == "welcome"
print(welcome)

c.send(json.dumps({
    "id": 1,
    "type": "subscribe",
    "topic": "/market/ticker:ETH-USDT",
    "privateChannel": False,
    "response": False
}))

while True:
    print(json.loads(c.recv()))
