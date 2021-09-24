#[macro_export]
macro_rules! ok {
    () => {
        hyper::Response::new(hyper::Body::from(""))
    };

    ($msg:expr) => {
        hyper::Response::new(hyper::Body::from($msg))
    };
}
pub(crate) use ok;

#[macro_export]
macro_rules! internal_error {
    () => {
        $crate::response_with_code!(500, "an error occurred during processing")
    };

    ($err:expr) => {
        $crate::response_with_code!(
            500,
            format!("an error occurred during processing: {}", $err)
        )
    };
}
pub(crate) use internal_error;

#[macro_export]
macro_rules! malformed {
    () => {
        $crate::response_with_code!(400, "malformed request")
    };
}
pub(crate) use malformed;

#[macro_export]
macro_rules! not_found {
    () => {
        $crate::response_with_code!(404, "not found")
    };
}
pub(crate) use not_found;

#[macro_export]
macro_rules! response_with_code {
    ($code:expr) => {
        hyper::Response::builder()
            .status($code)
            .body(hyper::Body::from(""))
            .unwrap()
    };

    ($code:expr, $msg:expr) => {
        hyper::Response::builder()
            .status($code)
            .body(hyper::Body::from($msg))
            .unwrap()
    };
}
pub(crate) use response_with_code;

#[macro_export]
macro_rules! request_auth {
    () => {
        hyper::Response::builder()
            .status(401)
            .header(hyper::header::WWW_AUTHENTICATE, "Basic")
            .body(hyper::Body::from("not authorized"))
            .unwrap()
    };
}
pub(crate) use request_auth;
