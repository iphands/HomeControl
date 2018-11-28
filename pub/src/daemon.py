from flask import Flask
from flask import jsonify
from awake import wol
import socket

app = Flask(__name__)

@app.route('/wake_cosmo')
def wake_cosmo():
    wol.send_magic_packet('D8:CB:8A:39:E2:BF')
    return jsonify({ "action": "woke cosmo" })

@app.route('/cosmo_status')
def cosmo_status():
    s = socket.socket()
    s.settimeout(0.100)

    try:
        s.connect(('cosmo.lan', 22))
    except Exception as e:
        return jsonify({ "status": "down" })
    finally:
        s.close()
    return jsonify({ "status": "up" })

app.run(host='0.0.0.0')
# app.run(ssl_context=('cert.pem', 'key.pem'))
