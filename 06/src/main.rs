
use tonic::{async_trait, Status, Code};
use crate::flight_booking_service_server::{FlightBookingService, FlightBookingServiceServer};
use std::collections::HashMap;

tonic::include_proto!("flightbooking");

struct FlightBookingServiceStruct {
    bookings: std::sync::Arc<tokio::sync::RwLock<HashMap<String, Option<FlightBooking>>>>,
}

type TReqEmpty = tonic::Request<EmptyMessage>;
type TReqBook  = tonic::Request<FlightBooking>;
type TResBook  = Result<tonic::Response<FlightBooking>, tonic::Status>;
type TResBookL = Result<tonic::Response<FlightBookingsList>, tonic::Status>;

#[async_trait]
impl FlightBookingService for FlightBookingServiceStruct {

    async fn list_bookings(&self, _request: TReqEmpty) -> TResBookL {
        let db = self.bookings.read().await;

        Ok(tonic::Response::new(
            FlightBookingsList {
                bookings: db.iter().filter(|(_,v)| **v != None)
                                   .map(|(_,v)| v.as_ref().unwrap().clone()).collect()
            }
        ))
    }

    async fn add_booking(&self, request: TReqBook) -> TResBook {
        let mut db = self.bookings.write().await;
        let key = (db.len() + 1).to_string();
        let new_booking = FlightBooking{ booking_id: key.clone(), ..request.get_ref().clone() };
        db.insert(key, Some(new_booking.clone()));
        
        Ok(tonic::Response::new(new_booking))
    }

    async fn update_booking(&self, request: TReqBook) -> TResBook {
        let mut db = self.bookings.write().await;
        match db.get_mut(&request.get_ref().booking_id) {
            Some(e) if *e != None => {
                *e = Some(request.get_ref().clone());
                return Ok(tonic::Response::new(request.get_ref().clone()))
            },
            _ => return Err(Status::new(Code::NotFound, "booking not found"))
        }
    }

    async fn remove_booking(&self, request: TReqBook) -> TResBook { 
        let mut db = self.bookings.write().await;
        match db.get_mut(&request.get_ref().booking_id) {
            Some(e) if *e != None => {
                let tmp = e.clone().unwrap();
                *e = None;
                return Ok(tonic::Response::new(tmp))
            },
            _ => return Err(Status::new(Code::NotFound, "booking not found"))
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), tonic::transport::Error> {
    let addr = ([0, 0, 0, 0], 3000).into();

    let service = FlightBookingServiceStruct{ bookings: std::sync::Arc::new(HashMap::new().into()) };

    tonic::transport::Server::builder()
        .add_service(FlightBookingServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
