#[macro_export]
macro_rules! client {
    ($x:expr) => {{
        Client::builder().user_agent($x).build()?
    }};
}