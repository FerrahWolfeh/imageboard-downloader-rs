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

#[macro_export]
macro_rules! extract_ext_from_url {
    ($x:expr) => {{
        let ext = $x.split('.').next_back().unwrap();
        ext.to_string()
    }};
}

#[macro_export]
macro_rules! finish_and_print_results {
    ($bars:expr, $self:expr, $auth_res:expr) => {{
        $bars.main.finish_and_clear();
        println!(
            "{} {} {}",
            $self
                .counters
                .downloaded_mtx
                .lock()
                .unwrap()
                .to_string()
                .bold()
                .blue(),
            "files".bold().blue(),
            "downloaded".bold()
        );

        if $auth_res.is_some() && $self.blacklisted_posts > 0 {
            println!(
                "{} {}",
                $self.blacklisted_posts.to_string().bold().red(),
                "posts with blacklisted tags were not downloaded."
                    .bold()
                    .red()
            )
        }
    }};
    ($bars:expr, $self:expr) => {{
        $bars.main.finish_and_clear();
        println!(
            "{} {} {}",
            $self
                .counters
                .downloaded_mtx
                .lock()
                .unwrap()
                .to_string()
                .bold()
                .blue(),
            "files".bold().blue(),
            "downloaded".bold()
        );
    }};
}
