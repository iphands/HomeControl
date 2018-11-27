from flask import Flask
from flask import jsonify
from awake import wol

app = Flask(__name__)

@app.route('/wake_cosmo')
def hello():
    mac = 'D8:CB:8A:39:E2:BF'
    wol.send_magic_packet(mac)
    return jsonify({
        "action": "woke cosmo"
    })

app.run(host='0.0.0.0')
# app.run(ssl_context=('cert.pem', 'key.pem'))
