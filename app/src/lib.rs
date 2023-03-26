use bindings::{
    http::{self, IncomingRequest, ResponseOutparam},
    incoming_http::IncomingHttp,
    streams,
};

struct Component;

impl IncomingHttp for Component {
    fn handle(request: IncomingRequest, _response_out: ResponseOutparam) {
        let headers = http::new_fields(&[("Content-Type", "text/plain")]);
        let response = http::new_outgoing_response(200, headers);
        let stream = http::outgoing_response_write(response).unwrap();
        streams::write(stream, "Hello world!".as_bytes()).unwrap();
        http::set_response_outparam(Ok(response)).unwrap();
    }
}

bindings::export!(Component);
