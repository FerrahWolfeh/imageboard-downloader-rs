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
macro_rules! initialize_progress_bars {
    ($len:expr, $x:expr) => {{
        let bar = ProgressBar::new($len).with_style(master_progress_style(&$x.progress_template()));
        bar.set_draw_target(ProgressDrawTarget::stderr_with_hz(60));
        bar.enable_steady_tick(Duration::from_millis(100));

        // Initialize the bars
        let multi = Arc::new(MultiProgress::new());
        let main = Arc::new(multi.add(bar));

        Arc::new(ProgressArcs { main, multi })
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
