
// This code runs at compile time, generates sources from .proto
fn main() {
    tonic_build::configure()
        .compile(&["proto/paymentservice.proto"], &["proto"])
        .unwrap();
    
    tonic_build::configure()
        .compile(&["proto/validationservice.proto"], &["proto"])
        .unwrap();
}
