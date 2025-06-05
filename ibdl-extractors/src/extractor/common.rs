use ibdl_common::log::debug;
use std::fmt::Display;

#[allow(dead_code)]
pub fn convert_tags_to_string<S>(tags: &[S]) -> (Vec<String>, String)
where
    S: ToString + Display,
{
    let mut strvec: Vec<String> = Vec::with_capacity(tags.len());
    for s in &mut strvec {
        let s1 = (*s).to_string();
        *s = s1;
    }
    let tag_string = strvec.join("+");

    debug!("Tag List: {tag_string}");
    (strvec, tag_string)
}
