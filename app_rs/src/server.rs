use crate::looper::{Looper, LooperState};
use crate::modes::get_available_modes;
use crate::opts::OptValue;
use actix_files::NamedFile;
use actix_web::{web, App, HttpResponse, HttpServer};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
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

fn process_opts_for_response(opts: HashMap<String, OptValue>) -> HashMap<String, OptValue> {
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
    response_opts
}

fn process_opts_from_request(mut opts: HashMap<String, OptValue>) -> HashMap<String, OptValue> {
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
    opts
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
    HttpResponse::Ok().json(DelayResponse {
        delay: state.get_delay(),
    })
}

async fn set_delay(data: web::Data<AppState>, req: web::Json<SetDelayRequest>) -> HttpResponse {
    let mut state = data.looper_state.lock().unwrap();
    state.set_delay(req.delay);
    HttpResponse::Ok().json(DelayResponse {
        delay: state.get_delay(),
    })
}

async fn get_opts(data: web::Data<AppState>) -> HttpResponse {
    let state = data.looper_state.lock().unwrap();
    let opts = state.get_opts();
    HttpResponse::Ok().json(OptsResponse {
        opts: process_opts_for_response(opts),
    })
}

async fn set_opts(data: web::Data<AppState>, req: web::Json<HashMap<String, OptValue>>) -> HttpResponse {
    let opts = process_opts_from_request(req.0.clone());
    let mut state = data.looper_state.lock().unwrap();
    state.set_opts(opts);
    let current_opts = state.get_opts();
    HttpResponse::Ok().json(OptsResponse {
        opts: process_opts_for_response(current_opts),
    })
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

// --- Per-strip endpoints ---

async fn get_strip_mode(data: web::Data<AppState>, path: web::Path<u8>) -> HttpResponse {
    let strip_id = path.into_inner();
    let state = data.looper_state.lock().unwrap();

    match state.get_strip_mode(strip_id) {
        Some(mode) => HttpResponse::Ok().json(ModeResponse { mode }),
        None => HttpResponse::NotFound().json(ErrorResponse {
            error: format!("strip {} not found", strip_id),
        }),
    }
}

async fn set_strip_mode(data: web::Data<AppState>, path: web::Path<u8>, req: web::Json<SetModeRequest>) -> HttpResponse {
    let strip_id = path.into_inner();
    let mut state = data.looper_state.lock().unwrap();

    match state.set_strip_mode(strip_id, &req.mode) {
        Some(mode) => HttpResponse::Ok().json(ModeResponse { mode }),
        None => HttpResponse::NotFound().json(ErrorResponse {
            error: format!("strip {} not found or invalid mode", strip_id),
        }),
    }
}

async fn get_strip_brightness(data: web::Data<AppState>, path: web::Path<u8>) -> HttpResponse {
    let strip_id = path.into_inner();
    let state = data.looper_state.lock().unwrap();

    match state.get_strip_brightness(strip_id) {
        Some(brightness) => HttpResponse::Ok().json(BrightnessResponse { brightness }),
        None => HttpResponse::NotFound().json(ErrorResponse {
            error: format!("strip {} not found", strip_id),
        }),
    }
}

async fn set_strip_brightness(
    data: web::Data<AppState>,
    path: web::Path<u8>,
    req: web::Json<SetBrightnessRequest>,
) -> HttpResponse {
    let strip_id = path.into_inner();
    let mut state = data.looper_state.lock().unwrap();

    match state.set_strip_brightness(strip_id, req.brightness) {
        Some(brightness) => HttpResponse::Ok().json(BrightnessResponse { brightness }),
        None => HttpResponse::NotFound().json(ErrorResponse {
            error: format!("strip {} not found", strip_id),
        }),
    }
}

async fn get_strip_delay(data: web::Data<AppState>, path: web::Path<u8>) -> HttpResponse {
    let strip_id = path.into_inner();
    let state = data.looper_state.lock().unwrap();

    match state.get_strip_delay(strip_id) {
        Some(delay) => HttpResponse::Ok().json(DelayResponse { delay }),
        None => HttpResponse::NotFound().json(ErrorResponse {
            error: format!("strip {} not found", strip_id),
        }),
    }
}

async fn set_strip_delay(data: web::Data<AppState>, path: web::Path<u8>, req: web::Json<SetDelayRequest>) -> HttpResponse {
    let strip_id = path.into_inner();
    let mut state = data.looper_state.lock().unwrap();

    match state.set_strip_delay(strip_id, req.delay) {
        Some(delay) => HttpResponse::Ok().json(DelayResponse { delay }),
        None => HttpResponse::NotFound().json(ErrorResponse {
            error: format!("strip {} not found", strip_id),
        }),
    }
}

async fn get_strip_opts(data: web::Data<AppState>, path: web::Path<u8>) -> HttpResponse {
    let strip_id = path.into_inner();
    let state = data.looper_state.lock().unwrap();

    match state.get_strip_opts(strip_id) {
        Some(opts) => HttpResponse::Ok().json(OptsResponse {
            opts: process_opts_for_response(opts),
        }),
        None => HttpResponse::NotFound().json(ErrorResponse {
            error: format!("strip {} not found", strip_id),
        }),
    }
}

async fn set_strip_opts(
    data: web::Data<AppState>,
    path: web::Path<u8>,
    req: web::Json<HashMap<String, OptValue>>,
) -> HttpResponse {
    let strip_id = path.into_inner();
    let opts = process_opts_from_request(req.0.clone());
    let mut state = data.looper_state.lock().unwrap();

    match state.set_strip_opts(strip_id, opts) {
        Some(current_opts) => HttpResponse::Ok().json(OptsResponse {
            opts: process_opts_for_response(current_opts),
        }),
        None => HttpResponse::NotFound().json(ErrorResponse {
            error: format!("strip {} not found", strip_id),
        }),
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

async fn index(static_dir: web::Data<PathBuf>) -> actix_web::Result<NamedFile> {
    let index_path = static_dir.join("index.html");
    Ok(NamedFile::open(index_path)?)
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

    let static_dir_data = web::Data::new(static_dir.clone());

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .app_data(static_dir_data.clone())
            // API routes - all prefixed with /api
            .route("/api/modes", web::get().to(get_modes))
            .route("/api/modes/", web::get().to(get_modes))
            .route("/api/modes/current", web::get().to(get_current_mode))
            .route("/api/modes/current/", web::get().to(get_current_mode))
            .route("/api/modes/current", web::post().to(set_current_mode))
            .route("/api/modes/current/", web::post().to(set_current_mode))
            .route("/api/brightness", web::get().to(get_brightness))
            .route("/api/brightness/", web::get().to(get_brightness))
            .route("/api/brightness", web::post().to(set_brightness))
            .route("/api/brightness/", web::post().to(set_brightness))
            .route("/api/delay", web::get().to(get_delay))
            .route("/api/delay/", web::get().to(get_delay))
            .route("/api/delay", web::post().to(set_delay))
            .route("/api/delay/", web::post().to(set_delay))
            .route("/api/opts", web::get().to(get_opts))
            .route("/api/opts/", web::get().to(get_opts))
            .route("/api/opts", web::post().to(set_opts))
            .route("/api/opts/", web::post().to(set_opts))
            .route("/api/strips", web::get().to(get_strips))
            .route("/api/strips/", web::get().to(get_strips))
            .route("/api/strips/{id}", web::post().to(configure_strip))
            .route("/api/strips/{id}/", web::post().to(configure_strip))
            // Per-strip endpoints
            .route("/api/strips/{id}/mode", web::get().to(get_strip_mode))
            .route("/api/strips/{id}/mode/", web::get().to(get_strip_mode))
            .route("/api/strips/{id}/mode", web::post().to(set_strip_mode))
            .route("/api/strips/{id}/mode/", web::post().to(set_strip_mode))
            .route("/api/strips/{id}/brightness", web::get().to(get_strip_brightness))
            .route("/api/strips/{id}/brightness/", web::get().to(get_strip_brightness))
            .route("/api/strips/{id}/brightness", web::post().to(set_strip_brightness))
            .route("/api/strips/{id}/brightness/", web::post().to(set_strip_brightness))
            .route("/api/strips/{id}/delay", web::get().to(get_strip_delay))
            .route("/api/strips/{id}/delay/", web::get().to(get_strip_delay))
            .route("/api/strips/{id}/delay", web::post().to(set_strip_delay))
            .route("/api/strips/{id}/delay/", web::post().to(set_strip_delay))
            .route("/api/strips/{id}/opts", web::get().to(get_strip_opts))
            .route("/api/strips/{id}/opts/", web::get().to(get_strip_opts))
            .route("/api/strips/{id}/opts", web::post().to(set_strip_opts))
            .route("/api/strips/{id}/opts/", web::post().to(set_strip_opts))
            .route("/api/looper", web::get().to(get_looper_state))
            .route("/api/looper/", web::get().to(get_looper_state))
            .route("/api/looper", web::post().to(control_looper))
            .route("/api/looper/", web::post().to(control_looper))
            // Root path serves index.html
            .route("/", web::get().to(index))
            // Static files served from root (css, js)
            .service(actix_files::Files::new("/", &static_dir).index_file("index.html"))
    })
    .bind("0.0.0.0:5000")?
    .run()
    .await
}
