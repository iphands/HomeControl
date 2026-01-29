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
        orig_opts = loop.get_opts()
        new_opts = request.get_json()
        for key in new_opts:
            if new_opts[key]["type"] == "bool":
                new_opts[key]["val"] = True if new_opts[key]["val"] == "true" else False
            if new_opts[key]["type"] == "int":
                try:
                    new_opts[key]["val"] = int(new_opts[key]["val"])
                except:
                    new_opts[key]["val"] = orig_opts.get(key, {}).get("val", 0)
        loop.set_opts(new_opts)

    # Fetch current opts (after any updates)
    current_opts = loop.get_opts()
    for key, opt in current_opts.items():
        if isinstance(opt, int):
            continue
        if opt["type"] == "color":
            rgb_hex = "#%02x%02x%02x" % tuple(opt["val"])
            opt["val"] = rgb_hex
    return jsonify({"opts": current_opts})


@app.route("/strips", methods=["GET"])
def strips():
    """Return information about configured LED strips."""
    return jsonify({"strips": loop.get_strips()})


@app.route("/strips/<int:strip_id>", methods=["POST"])
def configure_strip(strip_id):
    """Configure a strip's network settings (debug mode only).

    POST body:
        - hostname: UDP destination hostname
        - port: UDP destination port
    """
    data = request.get_json() or {}
    result = loop.configure_strip(
        strip_id,
        hostname=data.get("hostname"),
        port=data.get("port"),
    )
    return jsonify(result)


@app.route("/looper", methods=["GET", "POST"])
def looper():
    """Control the animation loop (debug mode only).

    GET: Returns current loop state
    POST: Control the loop with:
        - iterations: number of loop iterations to run
        - next_state: "pause" or "running"
    """
    if request.method == "GET":
        return jsonify(loop.get_loop_state())

    data = request.get_json() or {}
    result = loop.loop_control(
        iterations=data.get("iterations"),
        next_state=data.get("next_state"),
    )
    return jsonify(result)


def start_server(debug=False):
    if debug:
        loop.set_debug_mode(True)
    app.run(host="0.0.0.0", port=5000, debug=False)
