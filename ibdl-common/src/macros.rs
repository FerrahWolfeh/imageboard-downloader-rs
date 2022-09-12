#[macro_export]
macro_rules! client {
    ($x:expr) => {{
        Client::builder()
            .user_agent($x.user_agent())
            .build()
            .unwrap()
    }};
}

#[macro_export]
macro_rules! join_tags {
    ($x:expr) => {{
        let tl = $x.join("+");
        tl
    }};
}

#[macro_export]
macro_rules! extract_ext_from_url {
    ($x:expr) => {{
        let ext = $x.split('.').next_back().unwrap();
        ext.to_string()
    }};
}
