use actix_web::{web, App, HttpServer, HttpResponse};
use log::info;
use serde::Deserialize;
use std::sync::Arc;

use crate::models::*;
use crate::storage::Storage;

/// Application state shared across all HTTP handlers
pub struct AppState {
    pub storage: Arc<Storage>,
}

// ─── Query Parameters ────────────────────────────────────────────

#[derive(Deserialize)]
pub struct DateQuery {
    pub date: Option<String>,
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub date: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct CategoryUpdate {
    pub app_name: String,
    pub category: String,
}

// ─── Handlers ────────────────────────────────────────────────────

async fn health() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({ "status": "ok", "service": "screen-time-tracker" }))
}

async fn today_usage(state: web::Data<AppState>) -> HttpResponse {
    let data = state.storage.get_today_usage();
    HttpResponse::Ok().json(ApiResponse::success(data))
}

async fn weekly_usage(state: web::Data<AppState>) -> HttpResponse {
    let data = state.storage.get_weekly_usage();
    HttpResponse::Ok().json(ApiResponse::success(data))
}

async fn app_breakdown(
    state: web::Data<AppState>,
    query: web::Query<DateQuery>,
) -> HttpResponse {
    let data = state.storage.get_app_breakdown(query.date.as_deref());
    HttpResponse::Ok().json(ApiResponse::success(data))
}

async fn session_history(
    state: web::Data<AppState>,
    query: web::Query<HistoryQuery>,
) -> HttpResponse {
    let limit = query.limit.unwrap_or(100);
    let data = state.storage.get_session_history(query.date.as_deref(), limit);
    HttpResponse::Ok().json(ApiResponse::success(data))
}

async fn website_usage(
    state: web::Data<AppState>,
    query: web::Query<DateQuery>,
) -> HttpResponse {
    let data = state.storage.get_website_usage(query.date.as_deref());
    HttpResponse::Ok().json(ApiResponse::success(data))
}

async fn focus_score(
    state: web::Data<AppState>,
    query: web::Query<DateQuery>,
) -> HttpResponse {
    let data = state.storage.get_focus_score(query.date.as_deref());
    HttpResponse::Ok().json(ApiResponse::success(data))
}

async fn get_categories(state: web::Data<AppState>) -> HttpResponse {
    let data = state.storage.get_all_categories();
    HttpResponse::Ok().json(ApiResponse::success(data))
}

async fn update_category(
    state: web::Data<AppState>,
    body: web::Json<CategoryUpdate>,
) -> HttpResponse {
    state.storage.set_category(&body.app_name, &body.category);
    HttpResponse::Ok().json(serde_json::json!({ "ok": true }))
}

async fn browser_event(
    state: web::Data<AppState>,
    body: web::Json<BrowserTabEvent>,
) -> HttpResponse {
    let session = BrowserSession::new(
        body.url.clone(),
        body.title.clone(),
        body.domain.clone(),
    );
    state.storage.insert_browser_session(&session);
    HttpResponse::Ok().json(serde_json::json!({ "ok": true }))
}

// ─── Server ──────────────────────────────────────────────────────

/// Start the HTTP IPC server on localhost:9746
/// This port is chosen to be unlikely to conflict with other services.
pub async fn start_server(storage: Arc<Storage>) -> std::io::Result<()> {
    let bind_addr = "127.0.0.1:9746";
    info!("Starting IPC server on {}", bind_addr);

    let state = web::Data::new(AppState { storage });

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            // Health check
            .route("/health", web::get().to(health))
            // Core API endpoints
            .route("/today-usage", web::get().to(today_usage))
            .route("/weekly-usage", web::get().to(weekly_usage))
            .route("/app-breakdown", web::get().to(app_breakdown))
            .route("/session-history", web::get().to(session_history))
            .route("/website-usage", web::get().to(website_usage))
            .route("/focus-score", web::get().to(focus_score))
            // Category management
            .route("/categories", web::get().to(get_categories))
            .route("/categories", web::put().to(update_category))
            // Browser extension endpoint
            .route("/browser-event", web::post().to(browser_event))
    })
    .bind(bind_addr)?
    .workers(2) // Minimal workers for a local-only service
    .run()
    .await
}
