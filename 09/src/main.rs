use axum::{Extension, http::StatusCode, response::IntoResponse, Router, routing::get};
use std::{iter::Iterator, net::SocketAddr, sync::Arc, time::Duration};
use tokio::{sync::RwLock, task, time};

struct RemoteStatus { address: String, alive: bool }
type RSDB = Arc<RwLock<Vec<RemoteStatus>>>;

async fn get_response(remote: &RemoteStatus) -> Option<reqwest::Response> {
    reqwest::get(&remote.address).await.ok().filter(|e| e.status() == StatusCode::OK)
}

async fn refresh_remotes(remotes: &mut Vec<RemoteStatus>) {
    for r in remotes.iter_mut() { r.alive = get_response(r).await.is_some(); }
}

#[tokio::main]
async fn main() {
    // Create status database
    let status: RSDB = Arc::new(RwLock::new(vec![
        RemoteStatus { address: "http://ni-am.fit.cvut.cz:8888/MI-MDW-LastMinute1/list".into(), alive: false },
        RemoteStatus { address: "http://ni-am.fit.cvut.cz:8888/MI-MDW-LastMinute2/list".into(), alive: false },
        RemoteStatus { address: "http://ni-am.fit.cvut.cz:8888/MI-MDW-LastMinute3/list".into(), alive: false },
    ]));
    
    // Set up routing
    let app = Router::new().route("/lastMinute/list", get(balance)).layer(Extension(status.clone()));
    
    // Set up refresh loop
    task::spawn(async move {
        let mut interval = time::interval(Duration::from_millis(10_000));
        loop {
            {
                let mut s = status.write().await;
                refresh_remotes(&mut s).await;
            }
            interval.tick().await;
        }
    });

    // Start serving
    axum::Server::bind(&(SocketAddr::from(([0, 0, 0, 0], 3000))))
                 .serve(app.into_make_service()).await.unwrap();
}

async fn balance(Extension(s): Extension<RSDB>) -> impl IntoResponse {
    let s = s.read().await;
    // Try all remotes, "alive" first
    let r = 'resp: {
        for host in s.iter().filter(|e| e.alive) {
            if let Some(resp) = get_response(host).await {
                break 'resp Some(resp);
            }
        }
        for host in s.iter().filter(|e| !e.alive) {
            if let Some(resp) = get_response(host).await {
                break 'resp Some(resp);
            }
        }
        None
    };
    match r {
        Some(resp) => {
            let mut header_map = axum::http::HeaderMap::new();
            for h in resp.headers(){
                if h.0 != axum::http::header::TRANSFER_ENCODING {
                    header_map.insert(h.0, h.1.clone());
                }
            }
            
            (StatusCode::OK, header_map, resp.text().await.unwrap()).into_response()
        },
        None => StatusCode::SERVICE_UNAVAILABLE.into_response()
    }
}
