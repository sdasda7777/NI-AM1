use axum::{Extension, extract::BodyStream, Json, Router, routing::post};
use futures_util::{StreamExt, TryStreamExt}; use regex::Regex;
use std::{io::{Error, ErrorKind}, net::SocketAddr, sync::Arc}; use serde::Serialize;
use tokio_util::{codec::{FramedRead, LinesCodec}, io::StreamReader};

#[derive(Default, Serialize)]
struct Person { name: String, surname: String, }
#[derive(Default, Serialize)]
struct Booking { id: String, location: String, person: Person, }
struct Patterns { id: Regex, location: Regex, name: Regex, }

#[tokio::main]
async fn main() {
    // Set up regex patterns
    let patterns = Arc::new(Patterns{id:       Regex::new("^Tour id: \".+\"$").unwrap(),
                                     location: Regex::new("^Location: \".+\"$").unwrap(),
                                     name:     Regex::new("^Person: \".+ .+\"$").unwrap()});

    // Set up routing
    let app = Router::new()
                    .route("/transform", post(transform_formats))
                    .layer(Extension(patterns));

    // Start serving
    axum::Server::bind(&(SocketAddr::from(([0, 0, 0, 0], 3000))))
                 .serve(app.into_make_service()).await.unwrap();
}

// Transforms string in format 1 to format 2 (JSON)
async fn transform_formats(Extension(patterns): Extension<Arc<Patterns>>, body: BodyStream) -> Json<Booking> {
    let mut booking = Booking{..Default::default()};
    let mut line_reader = FramedRead::new(StreamReader::new(
                body.map_err(|e| Error::new(ErrorKind::Other, e.into_inner()))), LinesCodec::new());

    let mut reading_data: bool = false;
    while let Some(Ok(line)) = line_reader.next().await {
        if line.eq("===") { reading_data = !reading_data; }
        else if reading_data {
            match (line.find('"'), line.rfind('"')){
                (Some(l), Some(r)) => // find leftmost and rightmost '"'
                    if patterns.id.is_match(&line) {
                        booking.id = line[(l+1)..r].into();
                    } else if patterns.location.is_match(&line) {
                        booking.location = line[(l+1)..r].into();
                    } else if patterns.name.is_match(&line) {
                        let s = line[l..].find(' ').unwrap(); // find first ' ' after the first '"'
                        booking.person.name = line[(l+1)..(l+s)].into();
                        booking.person.surname = line[(l+s+1)..(r)].into();
                    },
                _ => {}
            }
        }
    };

    Json(booking)
}
