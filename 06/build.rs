
// This code runs at compile time, generates sources from .proto
fn main() {
    tonic_build::configure()
        .compile(&["proto/bookingservice.proto"], &["proto"])
        .unwrap();
}
