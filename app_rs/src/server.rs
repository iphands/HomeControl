//! HTTP API server for LED strip control.
//!
//! This module provides a REST API for controlling LED strips,
/// managing animation modes, brightness, and configuration.
use std::collections::HashMap;
use std::path::PathBuf;

use actix_files::NamedFile;
use actix_web::{http::StatusCode, middleware, web, App, HttpResponse, HttpServer, ResponseError};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::error::{Error, ErrorResponse};
use crate::looper::{Looper, LooperState};
use crate::modes::available_modes;
use crate::opts::OptValue;

/// Shared application state available to all request handlers.
struct AppState {
    /// Reference to the looper state for LED control.
    looper_state: std::sync::Arc<std::sync::Mutex<LooperState>>,
}

/// Request body for setting the animation mode.
#[derive(Deserialize)]
struct SetModeRequest {
    mode: String,
}

/// Request body for setting brightness.
#[derive(Deserialize)]
struct SetBrightnessRequest {
    brightness: u8,
}

/// Request body for setting animation delay.
#[derive(Deserialize)]
struct SetDelayRequest {
    delay: f64,
}

/// Request body for configuring a strip.
#[derive(Deserialize)]
struct ConfigureStripRequest {
    #[serde(default)]
    hostname: Option<String>,
    #[serde(default)]
    port: Option<u16>,
}

/// Request body for controlling the animation loop.
#[derive(Deserialize)]
struct LoopControlRequest {
    #[serde(default)]
    iterations: Option<i64>,
    #[serde(default)]
    next_state: Option<String>,
}

/// Response containing the current animation mode.
#[derive(Serialize)]
struct ModeResponse {
    mode: String,
}

/// Response containing brightness information.
#[derive(Serialize)]
struct BrightnessResponse {
    brightness: u8,
}

/// Response containing delay information.
#[derive(Serialize)]
struct DelayResponse {
    delay: f64,
}

/// Response containing animation options.
#[derive(Serialize)]
struct OptionsResponse {
    opts: HashMap<String, OptValue>,
}

/// Response containing list of strips.
#[derive(Serialize)]
struct StripsResponse {
    strips: Vec<crate::looper::StripInfo>,
}

/// Response containing strip configuration.
#[derive(Serialize)]
struct StripConfigResponse {
    #[serde(flatten)]
    info: crate::looper::StripInfo,
}

/// Custom error type for HTTP responses.
#[derive(Debug)]
struct HttpError(Error);

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for HttpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl ResponseError for HttpError {
    fn error_response(&self) -> HttpResponse {
        let (status, error_response) = match &self.0 {
            Error::StripNotFound(_) => (StatusCode::NOT_FOUND, ErrorResponse::from(self.0.clone())),
            Error::UnknownMode(_) => (StatusCode::BAD_REQUEST, ErrorResponse::from(self.0.clone())),
            Error::DebugModeRequired => (
                StatusCode::FORBIDDEN,
                ErrorResponse::new("Debug mode required for this operation"),
            ),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, ErrorResponse::new("Internal server error")),
        };

        HttpResponse::build(status).json(error_response)
    }
}

impl From<Error> for HttpError {
    fn from(err: Error) -> Self {
        Self(err)
    }
}

/// Converts options to response format with color hex conversion.
fn convert_options_for_response(opts: HashMap<String, OptValue>) -> HashMap<String, OptValue> {
    opts.into_iter()
        .map(|(key, opt)| {
            let converted = if opt.type_name == "color" {
                opt.to_hex_color()
                    .map(|hex| OptValue {
                        value: serde_json::Value::String(hex),
                        type_name: opt.type_name.clone(),
                    })
                    .unwrap_or(opt)
            } else {
                opt
            };
            (key, converted)
        })
        .collect()
}

/// Converts request options to internal format with type coercion.
fn convert_options_from_request(mut opts: HashMap<String, OptValue>) -> HashMap<String, OptValue> {
    for (_, opt) in opts.iter_mut() {
        match opt.type_name.as_str() {
            "bool" => {
                if let Some(s) = opt.value.as_str() {
                    opt.value = serde_json::Value::Bool(s == "true");
                }
            }
            "int" => {
                if let Some(s) = opt.value.as_str() {
                    if let Ok(n) = s.parse::<i64>() {
                        opt.value = serde_json::Value::Number(n.into());
                    }
                }
            }
            "color" => {
                if let Some(s) = opt.value.as_str() {
                    if let Ok(rgb) = OptValue::parse_color(&serde_json::Value::String(s.to_string())) {
                        *opt = OptValue::color(rgb);
                    }
                }
            }
            _ => {}
        }
    }
    opts
}

/// GET /api/modes - Returns list of available animation modes.
async fn get_modes() -> HttpResponse {
    HttpResponse::Ok().json(available_modes())
}

/// GET /api/modes/current - Returns the current global mode.
async fn get_current_mode(data: web::Data<AppState>) -> std::result::Result<HttpResponse, HttpError> {
    let state = data.looper_state.lock().map_err(|_| Error::LockPoisoned)?;
    Ok(HttpResponse::Ok().json(ModeResponse {
        mode: state.current_mode(),
    }))
}

/// POST /api/modes/current - Sets the global animation mode.
async fn set_current_mode(
    data: web::Data<AppState>,
    req: web::Json<SetModeRequest>,
) -> std::result::Result<HttpResponse, HttpError> {
    let mut state = data.looper_state.lock().map_err(|_| Error::LockPoisoned)?;
    state.set_mode(&req.mode)?;
    Ok(HttpResponse::Ok().json(ModeResponse {
        mode: state.current_mode(),
    }))
}

/// GET /api/brightness - Returns the current global brightness.
async fn get_brightness(data: web::Data<AppState>) -> std::result::Result<HttpResponse, HttpError> {
    let state = data.looper_state.lock().map_err(|_| Error::LockPoisoned)?;
    Ok(HttpResponse::Ok().json(BrightnessResponse {
        brightness: state.brightness(),
    }))
}

/// POST /api/brightness - Sets the global brightness.
async fn set_brightness(
    data: web::Data<AppState>,
    req: web::Json<SetBrightnessRequest>,
) -> std::result::Result<HttpResponse, HttpError> {
    let mut state = data.looper_state.lock().map_err(|_| Error::LockPoisoned)?;
    state.set_brightness(req.brightness);
    Ok(HttpResponse::Ok().json(BrightnessResponse {
        brightness: state.brightness(),
    }))
}

/// GET /api/delay - Returns the current global delay.
async fn get_delay(data: web::Data<AppState>) -> std::result::Result<HttpResponse, HttpError> {
    let state = data.looper_state.lock().map_err(|_| Error::LockPoisoned)?;
    Ok(HttpResponse::Ok().json(DelayResponse { delay: state.delay() }))
}

/// POST /api/delay - Sets the global delay.
async fn set_delay(data: web::Data<AppState>, req: web::Json<SetDelayRequest>) -> std::result::Result<HttpResponse, HttpError> {
    let mut state = data.looper_state.lock().map_err(|_| Error::LockPoisoned)?;
    state.set_delay(req.delay);
    Ok(HttpResponse::Ok().json(DelayResponse { delay: state.delay() }))
}

/// GET /api/opts - Returns the current global options.
async fn get_options(data: web::Data<AppState>) -> std::result::Result<HttpResponse, HttpError> {
    let state = data.looper_state.lock().map_err(|_| Error::LockPoisoned)?;
    Ok(HttpResponse::Ok().json(OptionsResponse {
        opts: convert_options_for_response(state.options()),
    }))
}

/// POST /api/opts - Sets the global options.
async fn set_options(
    data: web::Data<AppState>,
    req: web::Json<HashMap<String, OptValue>>,
) -> std::result::Result<HttpResponse, HttpError> {
    let opts = convert_options_from_request(req.0.clone());
    let mut state = data.looper_state.lock().map_err(|_| Error::LockPoisoned)?;
    state.set_options(opts);
    Ok(HttpResponse::Ok().json(OptionsResponse {
        opts: convert_options_for_response(state.options()),
    }))
}

/// GET /api/strips - Returns information about all strips.
async fn get_strips(data: web::Data<AppState>) -> std::result::Result<HttpResponse, HttpError> {
    let state = data.looper_state.lock().map_err(|_| Error::LockPoisoned)?;
    Ok(HttpResponse::Ok().json(StripsResponse {
        strips: state.strips_info(),
    }))
}

/// POST /api/strips/{id} - Configures a specific strip (debug only).
async fn configure_strip(
    data: web::Data<AppState>,
    path: web::Path<u8>,
    req: web::Json<ConfigureStripRequest>,
) -> std::result::Result<HttpResponse, HttpError> {
    let strip_id = path.into_inner();
    let mut state = data.looper_state.lock().map_err(|_| Error::LockPoisoned)?;

    let info = state.configure_strip(strip_id, req.hostname.clone(), req.port)?;
    Ok(HttpResponse::Ok().json(StripConfigResponse { info }))
}

// ==================== Per-Strip Endpoints ====================

/// GET /api/strips/{id}/mode - Returns mode for a specific strip.
async fn get_strip_mode(data: web::Data<AppState>, path: web::Path<u8>) -> std::result::Result<HttpResponse, HttpError> {
    let strip_id = path.into_inner();
    let state = data.looper_state.lock().map_err(|_| Error::LockPoisoned)?;

    match state.strip_mode(strip_id) {
        Some(mode) => Ok(HttpResponse::Ok().json(ModeResponse { mode })),
        None => Err(HttpError(Error::StripNotFound(strip_id))),
    }
}

/// POST /api/strips/{id}/mode - Sets mode for a specific strip.
async fn set_strip_mode(
    data: web::Data<AppState>,
    path: web::Path<u8>,
    req: web::Json<SetModeRequest>,
) -> std::result::Result<HttpResponse, HttpError> {
    let strip_id = path.into_inner();
    let mut state = data.looper_state.lock().map_err(|_| Error::LockPoisoned)?;

    state.set_strip_mode(strip_id, &req.mode)?;
    Ok(HttpResponse::Ok().json(ModeResponse {
        mode: state.strip_mode(strip_id).unwrap_or_default(),
    }))
}

/// GET /api/strips/{id}/brightness - Returns brightness for a specific strip.
async fn get_strip_brightness(data: web::Data<AppState>, path: web::Path<u8>) -> std::result::Result<HttpResponse, HttpError> {
    let strip_id = path.into_inner();
    let state = data.looper_state.lock().map_err(|_| Error::LockPoisoned)?;

    match state.strip_brightness(strip_id) {
        Some(brightness) => Ok(HttpResponse::Ok().json(BrightnessResponse { brightness })),
        None => Err(HttpError(Error::StripNotFound(strip_id))),
    }
}

/// POST /api/strips/{id}/brightness - Sets brightness for a specific strip.
async fn set_strip_brightness(
    data: web::Data<AppState>,
    path: web::Path<u8>,
    req: web::Json<SetBrightnessRequest>,
) -> std::result::Result<HttpResponse, HttpError> {
    let strip_id = path.into_inner();
    let mut state = data.looper_state.lock().map_err(|_| Error::LockPoisoned)?;

    let brightness = state.set_strip_brightness(strip_id, req.brightness)?;
    Ok(HttpResponse::Ok().json(BrightnessResponse { brightness }))
}

/// GET /api/strips/{id}/delay - Returns delay for a specific strip.
async fn get_strip_delay(data: web::Data<AppState>, path: web::Path<u8>) -> std::result::Result<HttpResponse, HttpError> {
    let strip_id = path.into_inner();
    let state = data.looper_state.lock().map_err(|_| Error::LockPoisoned)?;

    match state.strip_delay(strip_id) {
        Some(delay) => Ok(HttpResponse::Ok().json(DelayResponse { delay })),
        None => Err(HttpError(Error::StripNotFound(strip_id))),
    }
}

/// POST /api/strips/{id}/delay - Sets delay for a specific strip.
async fn set_strip_delay(
    data: web::Data<AppState>,
    path: web::Path<u8>,
    req: web::Json<SetDelayRequest>,
) -> std::result::Result<HttpResponse, HttpError> {
    let strip_id = path.into_inner();
    let mut state = data.looper_state.lock().map_err(|_| Error::LockPoisoned)?;

    let delay = state.set_strip_delay(strip_id, req.delay)?;
    Ok(HttpResponse::Ok().json(DelayResponse { delay }))
}

/// GET /api/strips/{id}/opts - Returns options for a specific strip.
async fn get_strip_options(data: web::Data<AppState>, path: web::Path<u8>) -> std::result::Result<HttpResponse, HttpError> {
    let strip_id = path.into_inner();
    let state = data.looper_state.lock().map_err(|_| Error::LockPoisoned)?;

    match state.strip_options(strip_id) {
        Some(opts) => Ok(HttpResponse::Ok().json(OptionsResponse {
            opts: convert_options_for_response(opts),
        })),
        None => Err(HttpError(Error::StripNotFound(strip_id))),
    }
}

/// POST /api/strips/{id}/opts - Sets options for a specific strip.
async fn set_strip_options(
    data: web::Data<AppState>,
    path: web::Path<u8>,
    req: web::Json<HashMap<String, OptValue>>,
) -> std::result::Result<HttpResponse, HttpError> {
    let strip_id = path.into_inner();
    let opts = convert_options_from_request(req.0.clone());
    let mut state = data.looper_state.lock().map_err(|_| Error::LockPoisoned)?;

    let current_opts = state.set_strip_options(strip_id, opts)?;
    Ok(HttpResponse::Ok().json(OptionsResponse {
        opts: convert_options_for_response(current_opts),
    }))
}

// ==================== Looper Control Endpoints ====================

/// GET /api/looper - Returns the current looper state.
async fn get_looper_state(data: web::Data<AppState>) -> std::result::Result<HttpResponse, HttpError> {
    let state = data.looper_state.lock().map_err(|_| Error::LockPoisoned)?;
    Ok(HttpResponse::Ok().json(state.loop_state()))
}

/// POST /api/looper - Controls the animation loop (debug only).
async fn control_looper(
    data: web::Data<AppState>,
    req: web::Json<LoopControlRequest>,
) -> std::result::Result<HttpResponse, HttpError> {
    let mut state = data.looper_state.lock().map_err(|_| Error::LockPoisoned)?;
    let result = state.control_loop(req.iterations, req.next_state.clone())?;
    Ok(HttpResponse::Ok().json(result))
}

// ==================== Static File Serving ====================

/// Serves the index.html file.
async fn serve_index(static_dir: web::Data<PathBuf>) -> actix_web::Result<NamedFile> {
    let index_path = static_dir.join("index.html");
    NamedFile::open(index_path).map_err(|e| {
        warn!("Failed to serve index.html: {}", e);
        actix_web::error::ErrorNotFound("index.html not found")
    })
}

/// Starts the HTTP server with all configured routes.
pub async fn start_server(looper: Looper) -> std::io::Result<()> {
    let app_state = web::Data::new(AppState {
        looper_state: looper.state(),
    });

    // Determine static file directory
    let static_dir = std::env::current_dir()
        .ok()
        .and_then(|p| {
            if p.file_name()?.to_str()? == "app_rs" {
                Some(p.parent()?.join("frontend"))
            } else {
                Some(p.join("frontend"))
            }
        })
        .unwrap_or_else(|| PathBuf::from("../frontend"));

    let static_dir_data = web::Data::new(static_dir.clone());

    info!("Starting HTTP server on 0.0.0.0:5000");
    info!("Serving static files from: {:?}", static_dir);

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .app_data(static_dir_data.clone())
            // Logging middleware
            .wrap(middleware::Logger::default())
            // Normalize paths middleware (handles trailing slashes)
            .wrap(middleware::NormalizePath::trim())
            // Error handling
            .app_data(web::JsonConfig::default().error_handler(|err, _req| {
                actix_web::error::InternalError::from_response(
                    err,
                    HttpResponse::BadRequest().json(ErrorResponse::new("Invalid JSON")),
                )
                .into()
            }))
            // API routes
            .service(
                web::scope("/api")
                    // Mode routes
                    .route("/modes", web::get().to(get_modes))
                    .route("/modes/current", web::get().to(get_current_mode))
                    .route("/modes/current", web::post().to(set_current_mode))
                    // Brightness routes
                    .route("/brightness", web::get().to(get_brightness))
                    .route("/brightness", web::post().to(set_brightness))
                    // Delay routes
                    .route("/delay", web::get().to(get_delay))
                    .route("/delay", web::post().to(set_delay))
                    // Options routes
                    .route("/opts", web::get().to(get_options))
                    .route("/opts", web::post().to(set_options))
                    // Strips routes
                    .route("/strips", web::get().to(get_strips))
                    .route("/strips/{id}", web::post().to(configure_strip))
                    // Per-strip routes
                    .route("/strips/{id}/mode", web::get().to(get_strip_mode))
                    .route("/strips/{id}/mode", web::post().to(set_strip_mode))
                    .route("/strips/{id}/brightness", web::get().to(get_strip_brightness))
                    .route("/strips/{id}/brightness", web::post().to(set_strip_brightness))
                    .route("/strips/{id}/delay", web::get().to(get_strip_delay))
                    .route("/strips/{id}/delay", web::post().to(set_strip_delay))
                    .route("/strips/{id}/opts", web::get().to(get_strip_options))
                    .route("/strips/{id}/opts", web::post().to(set_strip_options))
                    // Looper control routes
                    .route("/looper", web::get().to(get_looper_state))
                    .route("/looper", web::post().to(control_looper)),
            )
            // Root path
            .route("/", web::get().to(serve_index))
            // Static files
            .service(actix_files::Files::new("/", &static_dir).index_file("index.html"))
    })
    .bind("0.0.0.0:5000")?
    .run()
    .await
}
