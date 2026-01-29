import os
import looper as loop
import fan_ctrl as fan
from flask import Flask, request, json, jsonify, send_from_directory

app = Flask(__name__, static_folder='static')
app.config.from_object(__name__)
app.url_map.strict_slashes = False


def simple_get_set(request, key, getter, setter):
    if request.method == "POST":
        setter(request.get_json()[key])
    return jsonify({key: getter()})


def _process_opts_for_response(opts):
    """Convert options for API response (e.g., color arrays to hex)."""
    result = {}
    for key, opt in opts.items():
        if isinstance(opt, int):
            result[key] = opt
            continue
        if opt.get("type") == "color":
            rgb_hex = "#%02x%02x%02x" % tuple(opt["val"])
            result[key] = {**opt, "val": rgb_hex}
        else:
            result[key] = opt
    return result


def _process_opts_from_request(new_opts, orig_opts):
    """Process options from API request (e.g., hex to color arrays)."""
    for key in new_opts:
        if new_opts[key]["type"] == "bool":
            new_opts[key]["val"] = True if new_opts[key]["val"] == "true" else False
        if new_opts[key]["type"] == "int":
            try:
                new_opts[key]["val"] = int(new_opts[key]["val"])
            except:
                new_opts[key]["val"] = orig_opts.get(key, {}).get("val", 0)
    return new_opts


@app.route("/")
def hello_world():
    return send_from_directory('static', 'index.html')


@app.route("/fanbuttons/<string:btn_id>")
def fan_buttons(btn_id):
    if fan.send_op(btn_id):
        return jsonify({"msg": "success"})
    return jsonify({"msg": "error"}, 400)


@app.route("/api/modes", methods=["GET"])
def modes():
    return json.dumps(list(loop.get_modes().keys()))


@app.route("/api/modes/current", methods=["GET", "POST"])
def current_mode():
    return simple_get_set(request, "mode", loop.get_current_mode, loop.set_mode)


@app.route("/api/brightness", methods=["GET", "POST"])
def brightness():
    return simple_get_set(
        request, "brightness", loop.get_brightness, loop.set_brightness
    )


@app.route("/api/delay", methods=["GET", "POST"])
def delay():
    return simple_get_set(request, "delay", loop.get_delay, loop.set_delay)


@app.route("/api/opts", methods=["GET", "POST"])
def opts():
    if request.method == "POST":
        orig_opts = loop.get_opts()
        new_opts = request.get_json()
        new_opts = _process_opts_from_request(new_opts, orig_opts)
        loop.set_opts(new_opts)

    current_opts = loop.get_opts()
    return jsonify({"opts": _process_opts_for_response(current_opts)})


@app.route("/api/strips", methods=["GET"])
def strips():
    """Return information about configured LED strips with per-strip state."""
    return jsonify({"strips": loop.get_strips()})


@app.route("/api/strips/<int:strip_id>", methods=["POST"])
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


# --- Per-strip endpoints ---


@app.route("/api/strips/<int:strip_id>/mode", methods=["GET", "POST"])
def strip_mode(strip_id):
    """Get or set mode for a specific strip."""
    if request.method == "POST":
        data = request.get_json() or {}
        mode_name = data.get("mode")
        result = loop.set_strip_mode(strip_id, mode_name)
        if result is None:
            return (
                jsonify({"error": f"strip {strip_id} not found or invalid mode"}),
                404,
            )
        return jsonify({"mode": result})
    else:
        result = loop.get_strip_mode(strip_id)
        if result is None:
            return jsonify({"error": f"strip {strip_id} not found"}), 404
        return jsonify({"mode": result})


@app.route("/api/strips/<int:strip_id>/brightness", methods=["GET", "POST"])
def strip_brightness(strip_id):
    """Get or set brightness for a specific strip."""
    if request.method == "POST":
        data = request.get_json() or {}
        val = data.get("brightness")
        result = loop.set_strip_brightness(strip_id, val)
        if result is None:
            return jsonify({"error": f"strip {strip_id} not found"}), 404
        return jsonify({"brightness": result})
    else:
        result = loop.get_strip_brightness(strip_id)
        if result is None:
            return jsonify({"error": f"strip {strip_id} not found"}), 404
        return jsonify({"brightness": result})


@app.route("/api/strips/<int:strip_id>/delay", methods=["GET", "POST"])
def strip_delay(strip_id):
    """Get or set delay for a specific strip."""
    if request.method == "POST":
        data = request.get_json() or {}
        val = data.get("delay")
        result = loop.set_strip_delay(strip_id, val)
        if result is None:
            return jsonify({"error": f"strip {strip_id} not found"}), 404
        return jsonify({"delay": result})
    else:
        result = loop.get_strip_delay(strip_id)
        if result is None:
            return jsonify({"error": f"strip {strip_id} not found"}), 404
        return jsonify({"delay": result})


@app.route("/api/strips/<int:strip_id>/opts", methods=["GET", "POST"])
def strip_opts(strip_id):
    """Get or set options for a specific strip."""
    if request.method == "POST":
        orig_opts = loop.get_strip_opts(strip_id)
        if orig_opts is None:
            return jsonify({"error": f"strip {strip_id} not found"}), 404
        new_opts = request.get_json() or {}
        new_opts = _process_opts_from_request(new_opts, orig_opts)
        result = loop.set_strip_opts(strip_id, new_opts)
        return jsonify({"opts": _process_opts_for_response(result)})
    else:
        result = loop.get_strip_opts(strip_id)
        if result is None:
            return jsonify({"error": f"strip {strip_id} not found"}), 404
        return jsonify({"opts": _process_opts_for_response(result)})


@app.route("/api/looper", methods=["GET", "POST"])
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
