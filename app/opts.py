from colour import Color


def create_int(val):
    return {"val": int(val), "type": "int"}


def create_float(val):
    return {"val": float(val), "type": "float"}


def create_bool(val):
    return {"val": val, "type": "bool"}


def set_color(opt):
    tmp = opt["val"]
    if tmp.startswith("#"):
        opt["val"] = []
        for x in Color(tmp).rgb:
            opt["val"].append(int(x * 255))

    else:
        opt["val"] = map(int, opt["val"].split(","))
    return opt


def create_color(color):
    return {"val": color, "type": "color"}
