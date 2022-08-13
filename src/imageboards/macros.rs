#[macro_export]
macro_rules! client {
    ($x:expr) => {{
        Client::builder().user_agent($x).build()?
    }};
}

#[macro_export]
macro_rules! join_tags {
    ($x:expr) => {{
        let tl = $x.join("+");
        debug!("Tag List: {}", tl);
        tl
    }};
}
