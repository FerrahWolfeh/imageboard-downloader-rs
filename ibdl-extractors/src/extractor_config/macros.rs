#[macro_export]
macro_rules! server_config {
    ($name:expr, $pretty_name:expr, $server:expr, $client:expr, $ext:expr, $base_url:expr, $post_url:expr, $post_list_url:expr, $pool_idx_url:expr, $max_post_limit:expr, $auth_url:expr) => {
        ServerConfig {
            name: String::from($name),
            pretty_name: String::from($pretty_name),
            server: $server,
            client_user_agent: String::from($client),
            extractor_user_agent: String::from($ext),
            base_url: String::from($base_url),
            post_url: $post_url,
            post_list_url: Some(String::from($post_list_url)),
            pool_idx_url: $pool_idx_url,
            max_post_limit: $max_post_limit,
            auth_url: $auth_url,
        }
    };
}
