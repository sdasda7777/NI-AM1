= NI-AM1 HW05

== How to run

The server is written in Rust and can be run using `cargo run`.
Port to listen on can be specified in `src/main.rs`, in the `main` function.

Requests for testing purposes are in the `requests.http` file which can be run using `httpyac --all requests.http`
(potentially also some IDEs, but not completely sure about syntax compatibility).
Port to send requests to can be adjusted in the variable `port` in the prologue.

When executing through `httpyac`, id from the response when creating a tour is being read and following requests are formed accordingly.
There are also asserts set up, which mark the request as failed in case some part of the response is not as expected (last line of output summarizes number of successful and failed requests).

== Architecture

The API consists of 2 endpoints: `/tour` and `/tour/{id}`.

`/tour` accepts the GET and POST methods, allowing for listing of all tours and creation of a new tours.

`/tour/{id}` accepts the GET, PUT and DELETE methods, allowing for reading, updating and deletion of tours.
