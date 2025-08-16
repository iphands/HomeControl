import looper as loop
import fan_ctrl as fan
from flask import Flask, request, json, jsonify, redirect

app = Flask(__name__)
app.config.from_object(__name__)
app.url_map.strict_slashes = False


def simple_get_set(request, key, getter, setter):
    if request.method == "POST":
        setter(request.get_json()[key])
    return jsonify({key: getter()})


@app.route("/")
def hello_world():
    return redirect("/static/index.html")


@app.route("/fanbuttons/<string:btn_id>")
def fan_buttons(btn_id):
    if fan.send_op(btn_id):
        return jsonify({"msg": "success"})
    return jsonify({"msg": "error"}, 400)


@app.route("/modes", methods=["GET"])
def modes():
    return json.dumps(list(loop.get_modes().keys()))


@app.route("/modes/current", methods=["GET", "POST"])
def current_mode():
    return simple_get_set(request, "mode", loop.get_current_mode, loop.set_mode)


@app.route("/brightness", methods=["GET", "POST"])
def brightness():
    return simple_get_set(
        request, "brightness", loop.get_brightness, loop.set_brightness
    )


@app.route("/delay", methods=["GET", "POST"])
def delay():
    return simple_get_set(request, "delay", loop.get_delay, loop.set_delay)


@app.route("/opts", methods=["GET", "POST"])
def opts():
    if request.method == "POST":
        opts = request.get_json()
        for key in opts:
            if opts[key]["type"] == "bool":
                opts[key]["val"] = True if opts[key]["val"] == "true" else False
            if opts[key]["type"] == "int":
                try:
                    opts[key]["val"] = int(opts[key]["val"])
                except:
                    pass
        loop.set_opts(opts)
    return jsonify({"opts": loop.get_opts()})


def start_server():
    app.run(host="0.0.0.0", port=5000, debug=False)
    # app.run(host='0.0.0.0', port=5000, debug=True)
