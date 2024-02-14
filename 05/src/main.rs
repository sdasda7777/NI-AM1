use axum::{body::{Body, BoxBody, HttpBody}, Extension, extract::Path,
           http::{header::{self, HeaderValue}, Request, response::Parts, StatusCode},
           Json, middleware::{self, Next}, response::{Response, IntoResponse},
           Router, routing::{get}};
use chrono::{DateTime, Utc};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use serde::{Deserialize, Serialize};
use tokio::{sync::RwLock};
use etag::EntityTag;

/* `#[serde(default)]` means field is not required when deserializing */
#[derive(Clone, Deserialize, Serialize, PartialEq)]
struct Tour { #[serde(default)] id: u64, name: String, #[serde(default)] customers: Vec<u64> }

type MemDB = Arc<RwLock<HashMap<u64, (DateTime<Utc>, Option<Tour>)>>>;

// Based on alextes's implementation (https://gist.github.com/alextes/4095b1fca7d58bd3100825e10e882e57)
//   which already provided support for If-None-Match/ETag
//   but optimized for precomputed ETags and with support for If-Modified-Since/Last-Modified
async fn conditional_mw<B>(request: Request<B>, next: Next<B>) -> Result<Response, StatusCode> {
    let if_none_match_header = request.headers().get(header::IF_NONE_MATCH).cloned();
    let if_modified_since_header = request.headers().get(header::IF_MODIFIED_SINCE).cloned();
    // let path = request.uri().path().to_owned();
    
    let response = next.run(request).await;
    let (mut original_parts, mut original_body) = response.into_parts();
    let (etag, new_parts, new_body): (EntityTag, Parts, BoxBody) = {
        if let Some(et) = original_parts.headers.get(header::ETAG)
                                        .and_then(|e| e.to_str().unwrap().parse::<EntityTag>().ok()) {
            // response already has an ETag, assume it is better
            (et, original_parts, original_body)
        } else {
            let bytes = {
                let mut body_bytes = vec![];
                while let Some(inner) = original_body.data().await {
                    body_bytes.extend(inner.unwrap());
                }
                body_bytes
            };
            if bytes.len() == 0 {
                // no body => no ETag to compute, just return
                return Ok(original_parts.into_response());
            } else {
                // body is present, compute ETag
                let etag = EntityTag::from_data(&bytes);
                original_parts.headers.insert(
                    header::ETAG,
                    HeaderValue::from_str(&etag.to_string()).unwrap(),
                );
                (etag, original_parts, BoxBody::new(Body::from(bytes).map_err(axum::Error::new)))
            }
        }
    };
    
    match if_none_match_header {
        Some(if_none_match) => {
            // request has If-None-Match header, return 304 if valid and matching
            match if_none_match.to_str().unwrap().parse::<EntityTag>().ok().map(|e| etag.weak_eq(&e)) {
                Some(true) => Ok((StatusCode::NOT_MODIFIED, new_parts).into_response()),
                _ => Ok((new_parts, new_body).into_response())
            }
        },
        None => {
            // request has no If-None-Match, return 304 if If-Modified-Since is present, valid and outdated
            let parse = |h: &HeaderValue| DateTime::parse_from_rfc2822(h.to_str().unwrap()).ok();
            match if_modified_since_header.and_then(|e| parse(&e)).and_then(|if_modified_since|
                  new_parts.headers.get(header::LAST_MODIFIED).and_then(|e| parse(e)).map(|last_modified| if_modified_since >= last_modified)) {
                Some(true) => Ok((StatusCode::NOT_MODIFIED, new_parts).into_response()),
                _ => Ok((new_parts, new_body).into_response())
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Create "in-memory database"
    let db: MemDB = Arc::new(RwLock::new(HashMap::new()));

    // Set up routing
    let app = Router::new()
        .route("/tour", get(read_all_tours).post(create_tour))
        .route("/tour/:tour_id", get(read_tour).put(update_tour).delete(delete_tour))
        .layer(Extension(db))
        .layer(middleware::from_fn(conditional_mw));

    // Start serving
    axum::Server::bind(&(SocketAddr::from(([0, 0, 0, 0], 3000))))
                 .serve(app.into_make_service()).await.unwrap();
}

async fn read_all_tours(Extension(db): Extension<MemDB>) -> impl IntoResponse {
    let hash_map = db.read().await;
    let mut present_tours: Vec<_> = hash_map.iter().filter(|(_,v)| v.1.is_some())
                                                   .map(|(_,v)| v.1.clone().unwrap().clone()).collect();
    present_tours.sort_by(|l,r| l.id.partial_cmp(&r.id).unwrap());
    let last_modified = hash_map.iter().fold(DateTime::UNIX_EPOCH, |lhs,(_,v)| lhs.max(v.0));
    let mut weak_etag_vec = Vec::new();
    for e in &present_tours {
        weak_etag_vec.extend(e.id.to_le_bytes());
        weak_etag_vec.extend(e.name.as_bytes());
    }
    let mut etag = EntityTag::from_data(&weak_etag_vec); etag.weak = true;
    (StatusCode::OK, [("ETag", etag.to_string()),
                      ("Last-Modified", last_modified.to_rfc2822())], Json(present_tours)).into_response()
}

async fn create_tour(Extension(db): Extension<MemDB>, Json(tour_in): Json<Tour>) -> impl IntoResponse {
    let mut hash_map = db.write().await;
    let (datetime, new_tour) = (Utc::now(), Tour{ id: hash_map.len() as u64, ..tour_in });
    hash_map.insert(new_tour.id, (datetime, Some(new_tour.clone())));
    (StatusCode::CREATED, [("Last-Modified", datetime.to_rfc2822()),
                           ("Location", format!("/tour/{:?}", new_tour.id))], Json(new_tour))
}

async fn read_tour(Extension(db): Extension<MemDB>, Path(tour_id): Path<u64>) -> impl IntoResponse {
    let hash_map = db.read().await;
    match hash_map.get(&tour_id) {
        Some((datetime, Some(t))) =>
            (StatusCode::OK, [("Last-Modified", datetime.to_rfc2822())], Json(t)).into_response(),
        _ => StatusCode::NOT_FOUND.into_response()
    }
}

async fn update_tour(Extension(db): Extension<MemDB>, Path(tour_id): Path<u64>, Json(tour_in): Json<Tour>) -> impl IntoResponse {
    let mut hash_map = db.write().await;
    match hash_map.get_mut(&tour_id) {
        Some(ti) => {
            if ti.1 == None {
                StatusCode::NOT_FOUND.into_response()
            } else {
                let (datetime, new_tour) = (Utc::now(), Tour{ id: tour_id, ..tour_in });
                *ti = (datetime, Some(new_tour.clone()));
                (StatusCode::OK, [("Last-Modified", datetime.to_rfc2822()),
                                  ("Location", format!("/tour/{:?}", new_tour.id))], Json(new_tour)).into_response()
            }
        },
        _ => StatusCode::NOT_FOUND.into_response()
    }
}

async fn delete_tour(Extension(db): Extension<MemDB>, Path(tour_id): Path<u64>) -> impl IntoResponse {
    let mut hash_map = db.write().await;
    match hash_map.get_mut(&tour_id) {
        Some(ti) => {
            if ti.1 == None {
                StatusCode::NOT_FOUND
            } else {
                *ti = (Utc::now(), None);
                StatusCode::NO_CONTENT
            }
        },
        _ => StatusCode::NOT_FOUND
    }
}
