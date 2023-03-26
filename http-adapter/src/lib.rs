use std::{collections::HashMap, sync::Mutex};

use bindings::{
    fastly::{self, http_body_read, BodyHandle, Request, Response},
    http::{
        Error, Fields, FutureIncomingResponse, Headers, Http, IncomingRequest, IncomingResponse,
        IncomingStream, Method, OutgoingRequest, OutgoingResponse, OutgoingStream, Pollable,
        ResponseOutparam, Scheme, StatusCode, Trailers,
    },
    streams::{self, StreamError, Streams},
};
use once_cell::sync::Lazy;
use url::Url;

static STATE: Lazy<Mutex<State>> = Lazy::new(|| {
    let request = fastly::http_req_body_downstream_get().unwrap();
    Mutex::new(State {
        incoming_request: request,
        ..Default::default()
    })
});

#[derive(Default)]
struct State {
    incoming_request: Request,

    fields: HashMap<Fields, Vec<(String, String)>>,
    next_fields: Fields,

    incoming_streams: HashMap<IncomingStream, BodyHandle>,
    next_incoming_stream: IncomingStream,

    responses: HashMap<OutgoingResponse, Response>,
    next_response: OutgoingResponse,

    outgoing_streams: HashMap<OutgoingStream, BodyHandle>,
    next_outgoing_stream: OutgoingStream,

    response_outparam: Option<OutgoingResponse>,
}

struct Component;

impl Http for Component {
    fn drop_fields(fields: Fields) {
        let mut state = STATE.lock().unwrap();
        state.fields.remove(&fields).unwrap();
    }

    fn new_fields(entries: Vec<(String, String)>) -> Fields {
        let mut state = STATE.lock().unwrap();
        let fields = state.next_fields;
        state.next_fields += 1;
        state.fields.insert(fields, entries);
        fields
    }

    fn fields_get(fields: Fields, name: String) -> Vec<String> {
        let state = STATE.lock().unwrap();
        state
            .fields
            .get(&fields)
            .unwrap()
            .iter()
            .filter(|(k, _)| k == &name)
            .map(|(_, v)| v.clone())
            .collect()
    }

    fn fields_set(fields: Fields, name: String, value: Vec<String>) {
        let mut state = STATE.lock().unwrap();
        let entries = state.fields.get_mut(&fields).unwrap();
        entries.retain(|(k, _)| k != &name);
        for v in value {
            entries.push((name.clone(), v));
        }
    }

    fn fields_delete(fields: Fields, name: String) {
        let mut state = STATE.lock().unwrap();
        let entries = state.fields.get_mut(&fields).unwrap();
        entries.retain(|(k, _)| k != &name);
    }

    fn fields_append(fields: Fields, name: String, value: String) {
        let mut state = STATE.lock().unwrap();
        let entries = state.fields.get_mut(&fields).unwrap();
        entries.push((name, value));
    }

    fn fields_entries(fields: Fields) -> Vec<(String, String)> {
        let state = STATE.lock().unwrap();
        state.fields.get(&fields).unwrap().clone()
    }

    fn fields_clone(fields: Fields) -> Fields {
        let mut state = STATE.lock().unwrap();
        let entries = state.fields.get(&fields).unwrap().clone();
        let new_fields = state.next_fields;
        state.next_fields += 1;
        state.fields.insert(new_fields, entries);
        new_fields
    }

    fn finish_incoming_stream(s: IncomingStream) -> Option<Trailers> {
        let mut state = STATE.lock().unwrap();
        state.incoming_streams.remove(&s).unwrap();
        None
    }

    fn finish_outgoing_stream(_s: OutgoingStream, _trailers: Option<Trailers>) {}

    fn drop_incoming_request(_request: IncomingRequest) {}

    fn drop_outgoing_request(_request: OutgoingRequest) {
        unimplemented!()
    }

    fn incoming_request_method(request: IncomingRequest) -> Method {
        let method = fastly::http_req_method_get(request).unwrap();
        match method.as_str() {
            "GET" => Method::Get,
            "POST" => Method::Post,
            "PUT" => Method::Put,
            "DELETE" => Method::Delete,
            "HEAD" => Method::Head,
            "OPTIONS" => Method::Options,
            "PATCH" => Method::Patch,
            "TRACE" => Method::Trace,
            "CONNECT" => Method::Connect,
            _ => Method::Other(method),
        }
    }

    fn incoming_request_path(request: IncomingRequest) -> String {
        let url: Url = fastly::http_req_uri_get(request).unwrap().parse().unwrap();
        url.path().to_string()
    }

    fn incoming_request_query(request: IncomingRequest) -> String {
        let url: Url = fastly::http_req_uri_get(request).unwrap().parse().unwrap();
        url.query().unwrap_or("").to_string()
    }

    fn incoming_request_scheme(request: IncomingRequest) -> Option<Scheme> {
        let url: Url = fastly::http_req_uri_get(request).ok()?.parse().ok()?;
        Some(match url.scheme() {
            "http" => Scheme::Http,
            "https" => Scheme::Https,
            _ => Scheme::Other(url.scheme().to_string()),
        })
    }

    fn incoming_request_authority(request: IncomingRequest) -> String {
        let url: Url = fastly::http_req_uri_get(request).unwrap().parse().unwrap();
        url.host().map(|h| h.to_string()).unwrap_or_default()
    }

    fn incoming_request_headers(request: IncomingRequest) -> Headers {
        Self::new_fields(
            fastly::http_req_header_names_get(request)
                .unwrap()
                .into_iter()
                .map(|name| {
                    let value = fastly::http_req_header_value_get(request, &name)
                        .unwrap()
                        .unwrap_or_default();
                    (name, value)
                })
                .collect(),
        )
    }

    fn incoming_request_consume(request: IncomingRequest) -> Result<IncomingStream, ()> {
        let mut state = STATE.lock().unwrap();
        assert_eq!(request, state.incoming_request.0);
        let stream = state.next_incoming_stream;
        state.next_incoming_stream += 1;
        let body = state.incoming_request.1;
        state.incoming_streams.insert(stream, body);
        Ok(stream)
    }

    fn new_outgoing_request(
        _method: Method,
        _path: String,
        _query: String,
        _scheme: Option<Scheme>,
        _authority: String,
        _headers: Headers,
    ) -> OutgoingRequest {
        unimplemented!()
    }

    fn outgoing_request_write(_request: OutgoingRequest) -> Result<OutgoingStream, ()> {
        unimplemented!()
    }

    fn drop_response_outparam(response: ResponseOutparam) {
        assert_eq!(response, 0);
        let mut state = STATE.lock().unwrap();
        if let Some(response) = state.response_outparam {
            let (response, body) = state.responses.get(&response).unwrap();
            fastly::http_resp_send_downstream(*response, *body, false).unwrap()
        }
        state.response_outparam = None;
    }

    fn set_response_outparam(response: Result<OutgoingResponse, Error>) -> Result<(), ()> {
        let mut state = STATE.lock().unwrap();
        state.response_outparam = Some(response.unwrap());
        Ok(())
    }

    fn drop_incoming_response(_response: IncomingResponse) {
        unimplemented!()
    }

    fn drop_outgoing_response(response: OutgoingResponse) {
        let mut state = STATE.lock().unwrap();
        state.responses.remove(&response).unwrap();
    }

    fn incoming_response_status(_response: IncomingResponse) -> StatusCode {
        unimplemented!()
    }

    fn incoming_response_headers(_response: IncomingResponse) -> Headers {
        unimplemented!()
    }

    fn incoming_response_consume(_response: IncomingResponse) -> Result<IncomingStream, ()> {
        unimplemented!()
    }

    fn new_outgoing_response(status_code: StatusCode, headers: Headers) -> OutgoingResponse {
        let resp = fastly::http_resp_new().unwrap();

        fastly::http_resp_status_set(resp, status_code).unwrap();
        for (name, value) in Self::fields_entries(headers) {
            fastly::http_resp_header_append(resp, &name, &value).unwrap();
        }

        let mut state = STATE.lock().unwrap();
        let response = state.next_response;
        state.next_response += 1;
        state
            .responses
            .insert(response, (resp, fastly::http_body_new().unwrap()));
        response
    }

    fn outgoing_response_write(response: OutgoingResponse) -> Result<OutgoingStream, ()> {
        let mut state = STATE.lock().unwrap();
        let (_, body) = *state.responses.get(&response).unwrap();
        let stream = state.next_outgoing_stream;
        state.next_outgoing_stream += 1;
        state.outgoing_streams.insert(stream, body);
        Ok(stream)
    }

    fn drop_future_incoming_response(_f: FutureIncomingResponse) {
        unimplemented!()
    }

    fn future_incoming_response_get(
        _f: FutureIncomingResponse,
    ) -> Option<Result<IncomingResponse, Error>> {
        unimplemented!()
    }

    fn listen_to_future_incoming_response(_f: FutureIncomingResponse) -> Pollable {
        unimplemented!()
    }
}

impl Streams for Component {
    fn read(
        this: streams::InputStream,
        len: u64,
    ) -> Result<(wit_bindgen::rt::vec::Vec<u8>, bool), streams::StreamError> {
        let state = STATE.lock().unwrap();
        let body = state.incoming_streams.get(&this).unwrap();
        let bytes = http_body_read(*body, len as u32).map_err(|_| StreamError {})?;
        let empty = bytes.is_empty();
        Ok((bytes, empty))
    }

    fn blocking_read(
        this: streams::InputStream,
        len: u64,
    ) -> Result<(wit_bindgen::rt::vec::Vec<u8>, bool), streams::StreamError> {
        Self::read(this, len)
    }

    fn skip(_this: streams::InputStream, _len: u64) -> Result<(u64, bool), streams::StreamError> {
        unimplemented!()
    }

    fn blocking_skip(
        this: streams::InputStream,
        len: u64,
    ) -> Result<(u64, bool), streams::StreamError> {
        Self::skip(this, len)
    }

    fn subscribe_to_input_stream(_this: streams::InputStream) -> streams::Pollable {
        unimplemented!()
    }

    fn drop_input_stream(this: streams::InputStream) {
        let mut state = STATE.lock().unwrap();
        state.incoming_streams.remove(&this).unwrap();
    }

    fn write(
        this: streams::OutputStream,
        buf: wit_bindgen::rt::vec::Vec<u8>,
    ) -> Result<u64, streams::StreamError> {
        let state = STATE.lock().unwrap();
        let body = state.outgoing_streams.get(&this).unwrap();
        fastly::http_body_write(*body, &buf, fastly::BodyWriteEnd::Back)
            .map(|n| n as u64)
            .map_err(|_| StreamError {})
    }

    fn blocking_write(
        this: streams::OutputStream,
        buf: wit_bindgen::rt::vec::Vec<u8>,
    ) -> Result<u64, streams::StreamError> {
        Self::write(this, buf)
    }

    fn write_zeroes(_this: streams::OutputStream, _len: u64) -> Result<u64, streams::StreamError> {
        unimplemented!()
    }

    fn blocking_write_zeroes(
        this: streams::OutputStream,
        len: u64,
    ) -> Result<u64, streams::StreamError> {
        Self::write_zeroes(this, len)
    }

    fn splice(
        _this: streams::OutputStream,
        _src: streams::InputStream,
        _len: u64,
    ) -> Result<(u64, bool), streams::StreamError> {
        unimplemented!()
    }

    fn blocking_splice(
        this: streams::OutputStream,
        src: streams::InputStream,
        len: u64,
    ) -> Result<(u64, bool), streams::StreamError> {
        Self::splice(this, src, len)
    }

    fn forward(
        _this: streams::OutputStream,
        _src: streams::InputStream,
    ) -> Result<u64, streams::StreamError> {
        unimplemented!()
    }

    fn subscribe_to_output_stream(_this: streams::OutputStream) -> streams::Pollable {
        unimplemented!()
    }

    fn drop_output_stream(this: streams::OutputStream) {
        let mut state = STATE.lock().unwrap();
        state.outgoing_streams.remove(&this).unwrap();
    }
}

bindings::export!(Component);
