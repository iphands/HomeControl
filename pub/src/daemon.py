from functools import wraps
from flask import request, make_response
from flask import Flask
from flask import jsonify
from awake import wol
import os
import socket

app = Flask(__name__)
VALID_TOKEN = os.environ["HOME_TOKEN"]
CERT = "/ssl/home.ahands.org/fullchain.pem"
KEY = "/ssl/home.ahands.org/privkey.pem"


def token_required(f):
    @wraps(f)
    def decorated_function(*args, **kwargs):
        if not "token" in request.args:
            return make_response(
                jsonify({"msg": "Unauthenticated, no token provided", "status": "401"}),
                401,
            )
        if "deadbeef12" != request.args["token"]:
            return make_response(
                jsonify({"msg": "Unauthorized, invalid token", "status": "403"}), 403
            )
        return f(*args, **kwargs)

    return decorated_function


@app.route("/wake_cosmo")
@token_required
def wake_cosmo():
    wol.send_magic_packet("D8:CB:8A:39:E2:BF")
    return jsonify({"action": "woke cosmo"})


@app.route("/cosmo_status")
@token_required
def cosmo_status():
    s = socket.socket()
    s.settimeout(0.100)

    try:
        s.connect(("cosmo.lan", 22))
    except Exception as e:
        return jsonify({"status": "down"})
    finally:
        s.close()
    return jsonify({"status": "up"})


app.run(host="0.0.0.0", ssl_context=(CERT, KEY))
