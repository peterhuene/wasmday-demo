use bindings::{export, fastly, CommandAdapter, Descriptor, InputStream, OutputStream, http_incoming_handler::handle, http};

struct Component;

impl CommandAdapter for Component {
    fn main(
        _stdin: InputStream,
        _stdout: OutputStream,
        _stderr: OutputStream,
        _args: Vec<String>,
        _preopens: Vec<(Descriptor, String)>,
    ) -> Result<(), ()> {
        fastly::abi_init(1).map_err(|_| ())?;
        let (request, _) = fastly::http_req_body_downstream_get().map_err(|_| ())?;
        handle(request, 0);
        http::drop_response_outparam(0);
        Ok(())
    }
}

export!(Component);
