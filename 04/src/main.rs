use axum::{Extension, extract::Path, http::StatusCode, Json, response::IntoResponse,
Router, routing::{get, post}}; use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use serde::{Deserialize, Serialize}; use tokio::{sync::RwLock};

#[derive(Clone, Default, Deserialize, Serialize)]
struct Tour { #[serde(default)] /*= not required when deserializing*/ id: u64, name: String, }
enum TW { Present(Tour), ToDelete(Tour), Deleted(Tour) }
#[derive(Deserialize)]
struct Confirmation { tour_id: u64 }

#[tokio::main]
async fn main() {
    // Create "in-memory database"
    let db: Arc<RwLock<HashMap<u64, TW>>> = Arc::new(RwLock::new(HashMap::new()));

    // Set up routing
    let app = Router::new()
        .route("/tour", post(create_tour))
        .route("/tour/:tour_id", get(read_tour).delete(delete_tour))
        .route("/confirmation", post(confirm_delete))
        .layer(Extension(db));

    // Start serving
    axum::Server::bind(&(SocketAddr::from(([0, 0, 0, 0], 3000))))
                 .serve(app.into_make_service()).await.unwrap();
}

async fn create_tour(Extension(db): Extension<Arc<RwLock<HashMap<u64, TW>>>>,
                     Json(tour): Json<Tour>) -> impl IntoResponse {
    let mut hash_map = db.write().await;
    let new_tour = Tour{id: hash_map.len() as u64, ..tour};
    hash_map.insert(new_tour.id, TW::Present(new_tour.clone()));
    (StatusCode::CREATED, [("Location", format!("/tours/{:?}", new_tour.id))], Json(new_tour))
}

async fn read_tour(Extension(db): Extension<Arc<RwLock<HashMap<u64, TW>>>>,
                   Path(tour_id): Path<u64>) -> impl IntoResponse {
    let hash_map = db.read().await;
    match hash_map.get(&tour_id) {
        Some(TW::Present(t)) | Some(TW::ToDelete(t)) =>
            (StatusCode::OK, [("Location", format!("/tours/{:?}", tour_id))], Json(t)).into_response(),
        _ => (StatusCode::NOT_FOUND, [("Location", format!("/tours/{:?}", tour_id))]).into_response()
    }
}

async fn delete_tour(Extension(db): Extension<Arc<RwLock<HashMap<u64, TW>>>>,
                     Path(tour_id): Path<u64>) -> impl IntoResponse {
    let mut hash_map = db.write().await;
    match hash_map.get(&tour_id) {
        Some(tw) => match tw {
            TW::Present(t) => {
                let tc: Tour = t.clone();
                hash_map.insert(tour_id, TW::ToDelete(tc.clone()));
                (StatusCode::OK, [("Location", format!("/tours/{:?}", tour_id))], Json(tc)).into_response()
            },
            TW::ToDelete(t) =>
                (StatusCode::OK, [("Location", format!("/tours/{:?}", tour_id))], Json(t)).into_response(),
            TW::Deleted(_) =>
                (StatusCode::NO_CONTENT, [("Location", format!("/tours/{:?}", tour_id))]).into_response()
        },
        _ => (StatusCode::NOT_FOUND, [("Location", format!("/tours/{:?}", tour_id))]).into_response()
    }
}

async fn confirm_delete(Extension(db): Extension<Arc<RwLock<HashMap<u64, TW>>>>,
                        Json(c): Json<Confirmation>) -> impl IntoResponse {
    let mut hash_map = db.write().await;
    match hash_map.get(&c.tour_id) {
        Some(tw) => match tw {
            TW::ToDelete(t) => {
                let tc: Tour = t.clone();
                hash_map.insert(c.tour_id, TW::Deleted(tc));
                (StatusCode::NO_CONTENT, [("Location", format!("/tours/{:?}", c.tour_id))])
            },
            TW::Deleted(_) =>
                (StatusCode::NO_CONTENT, [("Location", format!("/tours/{:?}", c.tour_id))]),
            TW::Present(_) =>
                (StatusCode::BAD_REQUEST, [("Location", format!("/tours/{:?}", c.tour_id))]),
        },
        _ => (StatusCode::NOT_FOUND, [("Location", format!("/tours/{:?}", c.tour_id))])
    }
}
