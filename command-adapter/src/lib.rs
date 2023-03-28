use bindings::{export, fastly, CommandAdapter, http_incoming_handler::handle, http};

struct Component;

impl CommandAdapter for Component {
    fn run() -> Result<(), ()> {
        fastly::abi_init(1).map_err(|_| ())?;
        let (request, _) = fastly::http_req_body_downstream_get().map_err(|_| ())?;
        handle(request, 0);
        http::drop_response_outparam(0);
        Ok(())
    }
}

export!(Component);
