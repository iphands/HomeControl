use crate::looper::{Looper, LooperState};
use crate::modes::get_available_modes;
use crate::opts::OptValue;
use actix_files::Files;
use actix_web::{web, App, HttpResponse, HttpServer};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct AppState {
    looper_state: Arc<Mutex<LooperState>>,
}

#[derive(Deserialize)]
struct SetModeRequest {
    mode: String,
}

#[derive(Deserialize)]
struct SetBrightnessRequest {
    brightness: u8,
}

#[derive(Deserialize)]
struct SetDelayRequest {
    delay: f64,
}

#[derive(Deserialize)]
struct ConfigureStripRequest {
    #[serde(default)]
    hostname: Option<String>,
    #[serde(default)]
    port: Option<u16>,
}

#[derive(Deserialize)]
struct LoopControlRequest {
    #[serde(default)]
    iterations: Option<i64>,
    #[serde(default)]
    next_state: Option<String>,
}

#[derive(Serialize)]
struct ModeResponse {
    mode: String,
}

#[derive(Serialize)]
struct BrightnessResponse {
    brightness: u8,
}

#[derive(Serialize)]
struct DelayResponse {
    delay: f64,
}

#[derive(Serialize)]
struct OptsResponse {
    opts: HashMap<String, OptValue>,
}

#[derive(Serialize)]
struct StripsResponse {
    strips: Vec<crate::looper::StripInfo>,
}

#[derive(Serialize)]
struct StripConfigResponse {
    #[serde(flatten)]
    info: crate::looper::StripInfo,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

async fn get_modes(_data: web::Data<AppState>) -> HttpResponse {
    let modes = get_available_modes();
    HttpResponse::Ok().json(modes)
}

async fn get_current_mode(data: web::Data<AppState>) -> HttpResponse {
    let state = data.looper_state.lock().unwrap();
    HttpResponse::Ok().json(ModeResponse {
        mode: state.get_current_mode_name(),
    })
}

async fn set_current_mode(data: web::Data<AppState>, req: web::Json<SetModeRequest>) -> HttpResponse {
    let mut state = data.looper_state.lock().unwrap();
    state.set_mode(&req.mode);
    HttpResponse::Ok().json(ModeResponse {
        mode: state.get_current_mode_name(),
    })
}

async fn get_brightness(data: web::Data<AppState>) -> HttpResponse {
    let state = data.looper_state.lock().unwrap();
    HttpResponse::Ok().json(BrightnessResponse {
        brightness: state.get_brightness(),
    })
}

async fn set_brightness(data: web::Data<AppState>, req: web::Json<SetBrightnessRequest>) -> HttpResponse {
    let mut state = data.looper_state.lock().unwrap();
    state.set_brightness(req.brightness);
    HttpResponse::Ok().json(BrightnessResponse {
        brightness: state.get_brightness(),
    })
}

async fn get_delay(data: web::Data<AppState>) -> HttpResponse {
    let state = data.looper_state.lock().unwrap();
    HttpResponse::Ok().json(DelayResponse { delay: state.delay })
}

async fn set_delay(data: web::Data<AppState>, req: web::Json<SetDelayRequest>) -> HttpResponse {
    let mut state = data.looper_state.lock().unwrap();
    state.delay = req.delay;
    HttpResponse::Ok().json(DelayResponse { delay: state.delay })
}

async fn get_opts(data: web::Data<AppState>) -> HttpResponse {
    let state = data.looper_state.lock().unwrap();
    let opts = state.get_opts();

    // Convert color arrays to hex strings for response
    let mut response_opts: HashMap<String, OptValue> = HashMap::new();
    for (key, opt) in opts {
        let converted_opt = if opt.type_name == "color" {
            if let Some(rgb) = crate::opts::parse_color(&opt.value) {
                let hex = format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]);
                OptValue {
                    value: serde_json::Value::String(hex),
                    type_name: opt.type_name,
                }
            } else {
                opt
            }
        } else {
            opt
        };
        response_opts.insert(key, converted_opt);
    }

    HttpResponse::Ok().json(OptsResponse { opts: response_opts })
}

async fn set_opts(data: web::Data<AppState>, req: web::Json<HashMap<String, OptValue>>) -> HttpResponse {
    let mut opts = req.0.clone();

    // Parse color values
    for (_, opt) in opts.iter_mut() {
        if opt.type_name == "bool" {
            if let Some(s) = opt.value.as_str() {
                opt.value = serde_json::Value::Bool(s == "true");
            }
        } else if opt.type_name == "int" {
            if let Some(s) = opt.value.as_str() {
                if let Ok(n) = s.parse::<i64>() {
                    opt.value = serde_json::Value::Number(n.into());
                }
            }
        } else if opt.type_name == "color" {
            if let Some(s) = opt.value.as_str() {
                if s.starts_with('#') && s.len() == 7 {
                    if let (Ok(r), Ok(g), Ok(b)) = (
                        u8::from_str_radix(&s[1..3], 16),
                        u8::from_str_radix(&s[3..5], 16),
                        u8::from_str_radix(&s[5..7], 16),
                    ) {
                        opt.value = serde_json::Value::Array(vec![
                            serde_json::Value::Number(r.into()),
                            serde_json::Value::Number(g.into()),
                            serde_json::Value::Number(b.into()),
                        ]);
                    }
                }
            }
        }
    }

    let mut state = data.looper_state.lock().unwrap();
    state.set_opts(opts);

    // Get updated opts and convert colors back to hex
    let current_opts = state.get_opts();
    let mut response_opts: HashMap<String, OptValue> = HashMap::new();
    for (key, opt) in current_opts {
        let converted_opt = if opt.type_name == "color" {
            if let Some(rgb) = crate::opts::parse_color(&opt.value) {
                let hex = format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]);
                OptValue {
                    value: serde_json::Value::String(hex),
                    type_name: opt.type_name,
                }
            } else {
                opt
            }
        } else {
            opt
        };
        response_opts.insert(key, converted_opt);
    }

    HttpResponse::Ok().json(OptsResponse { opts: response_opts })
}

async fn get_strips(data: web::Data<AppState>) -> HttpResponse {
    let state = data.looper_state.lock().unwrap();
    HttpResponse::Ok().json(StripsResponse {
        strips: state.get_strips(),
    })
}

async fn configure_strip(
    data: web::Data<AppState>,
    path: web::Path<u8>,
    req: web::Json<ConfigureStripRequest>,
) -> HttpResponse {
    let strip_id = path.into_inner();
    let mut state = data.looper_state.lock().unwrap();

    match state.configure_strip(strip_id, req.hostname.clone(), req.port) {
        Ok(info) => HttpResponse::Ok().json(StripConfigResponse { info }),
        Err(e) => HttpResponse::Ok().json(ErrorResponse { error: e }),
    }
}

async fn get_looper_state(data: web::Data<AppState>) -> HttpResponse {
    let state = data.looper_state.lock().unwrap();
    HttpResponse::Ok().json(state.get_loop_state())
}

async fn control_looper(data: web::Data<AppState>, req: web::Json<LoopControlRequest>) -> HttpResponse {
    let mut state = data.looper_state.lock().unwrap();
    let result = state.loop_control(req.iterations, req.next_state.clone());
    HttpResponse::Ok().json(result)
}

async fn hello() -> HttpResponse {
    HttpResponse::PermanentRedirect()
        .append_header(("location", "/static/index.html"))
        .finish()
}

pub async fn start_server(looper: Looper) -> std::io::Result<()> {
    let app_state = web::Data::new(AppState {
        looper_state: looper.get_state(),
    });

    // Get the project root and static directory path
    let static_dir = std::env::current_dir()
        .ok()
        .and_then(|p| {
            // If we're in app_rs, go up one level then into app/static
            if p.file_name()?.to_str()? == "app_rs" {
                Some(p.parent()?.join("app").join("static"))
            } else {
                // Otherwise assume we're in project root
                Some(p.join("app").join("static"))
            }
        })
        .unwrap_or_else(|| std::path::PathBuf::from("../app/static"));

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/", web::get().to(hello))
            .service(Files::new("/static", &static_dir).show_files_listing())
            .route("/modes", web::get().to(get_modes))
            .route("/modes/", web::get().to(get_modes))
            .route("/modes/current", web::get().to(get_current_mode))
            .route("/modes/current/", web::get().to(get_current_mode))
            .route("/modes/current", web::post().to(set_current_mode))
            .route("/modes/current/", web::post().to(set_current_mode))
            .route("/brightness", web::get().to(get_brightness))
            .route("/brightness/", web::get().to(get_brightness))
            .route("/brightness", web::post().to(set_brightness))
            .route("/brightness/", web::post().to(set_brightness))
            .route("/delay", web::get().to(get_delay))
            .route("/delay/", web::get().to(get_delay))
            .route("/delay", web::post().to(set_delay))
            .route("/delay/", web::post().to(set_delay))
            .route("/opts", web::get().to(get_opts))
            .route("/opts/", web::get().to(get_opts))
            .route("/opts", web::post().to(set_opts))
            .route("/opts/", web::post().to(set_opts))
            .route("/strips", web::get().to(get_strips))
            .route("/strips/", web::get().to(get_strips))
            .route("/strips/{id}", web::post().to(configure_strip))
            .route("/strips/{id}/", web::post().to(configure_strip))
            .route("/looper", web::get().to(get_looper_state))
            .route("/looper/", web::get().to(get_looper_state))
            .route("/looper", web::post().to(control_looper))
            .route("/looper/", web::post().to(control_looper))
    })
    .bind("0.0.0.0:5000")?
    .run()
    .await
}
