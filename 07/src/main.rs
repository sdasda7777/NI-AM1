
use tonic::{async_trait, Status, Code, transport::Endpoint};
use crate::payment_service_server::{PaymentService, PaymentServiceServer};
use crate::cards_service_client::{CardsServiceClient};
use std::collections::HashMap;

tonic::include_proto!("payments");
tonic::include_proto!("model");

struct PaymentServiceStruct {
    payments: std::sync::Arc<tokio::sync::RwLock<HashMap<String, Payment>>>,
}

type TReqEmpty = tonic::Request<EmptyMessage>;
type TReqPay   = tonic::Request<Payment>;
type TResPay   = Result<tonic::Response<Payment>, tonic::Status>;
type TResPayL  = Result<tonic::Response<PaymentList>, tonic::Status>;

#[async_trait]
impl PaymentService for PaymentServiceStruct {

    async fn list_all_payments(&self, _: TReqEmpty) -> TResPayL {
        let db = self.payments.read().await;

        Ok(tonic::Response::new(
            PaymentList {
                payments: db.iter().map(|(_,v)| v.clone()).collect()
            }
        ))
    }

    async fn process_payment(&self, request: TReqPay) -> TResPay {
        let mut db = self.payments.write().await;
        
        if request.get_ref().order_id == "" {
            return Err(Status::new(Code::InvalidArgument, "order id must not be empty"));
        }
        
        if db.get(&request.get_ref().order_id) != None {
            return Err(Status::new(Code::AlreadyExists, "order id must be unique"));
        }
        
        let card = Card { card_number: request.get_ref().card_number.clone(),
                          card_owner:  request.get_ref().card_owner.clone() };
        let endpoint = Endpoint::from_static("http://ni-am.fit.cvut.cz:9090");
        let mut client = match CardsServiceClient::connect(endpoint).await {
            Err(_) => return Err(Status::new(Code::Internal, "validation unreachable")),
            Ok(c) => c
        };
        
        match client.validate_card(card).await {
            Ok(r) if *r.get_ref() == true => {},
            Ok(_) => return Err(Status::new(Code::InvalidArgument, "payment details are invalid")),
            Err(e) => {
                println!("{:?}", e);
                return Err(Status::new(Code::Internal, "validation failure"))
            }
        }
        
        db.insert(request.get_ref().order_id.clone(), request.get_ref().clone());
        
        Ok(tonic::Response::new(request.get_ref().clone()))
    }
}

#[tokio::main]
async fn main() -> Result<(), tonic::transport::Error> {
    let addr = ([0, 0, 0, 0], 3000).into();

    let service = PaymentServiceStruct{ payments: std::sync::Arc::new(HashMap::new().into()) };

    tonic::transport::Server::builder()
        .add_service(PaymentServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
