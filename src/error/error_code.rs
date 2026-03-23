#![allow(dead_code)]

macro_rules! status_error_codes {
    // 匹配多个元组，生成多个常量
    (
        $(
            ($num:expr, $konst:ident, $phrase:expr);
        )+
    ) => {
        $(
            // 为每个元组生成一个常量定义
            pub const $konst: (i16, &str) = ($num, $phrase);
        )+
    }
}

status_error_codes! {
    (-1, SERVER_ERROR, "server error");
    (-2, INVALID_PARAMS, "invalid request parameters");
    (-3, UNAUTHORIZED, "unauthorized");
    (-4, FORBIDDEN, "permission denied");
    (-5, NOT_FOUND, "resource not found");
    (-6, CONFLICT, "resource conflict");
    (-7, TOO_MANY_REQUESTS, "too many requests");
    (-8, SERVICE_UNAVAILABLE, "service unavailable");
    (-9, BAD_GATEWAY, "bad gateway");
    (-10, GATEWAY_TIMEOUT, "gateway timeout");
}
