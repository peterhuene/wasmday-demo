use bindings::{
    svelte_demo_app::{render, AppPropsParam},
    http::{self, IncomingRequest, ResponseOutparam},
    incoming_http::IncomingHttp,
    streams,
};

struct Component;

impl IncomingHttp for Component {
    fn handle(_: IncomingRequest, _: ResponseOutparam) {
        let html = render(AppPropsParam {
            name: "Cloud Native Wasm Day!",
            count: 5,
        });

        let headers = http::new_fields(&[("Content-Type", "text/html")]);
        let response = http::new_outgoing_response(200, headers);
        let stream = http::outgoing_response_write(response).unwrap();
        streams::write(stream, html.as_bytes()).unwrap();
        http::set_response_outparam(Ok(response)).unwrap();
    }
}

bindings::export!(Component);
