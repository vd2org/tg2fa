use shadow_rs;

fn main() {
    let tz = std::env::var("TZ").unwrap_or("UTC".to_string());
    std::env::set_var("TZ", "UTC");
    shadow_rs::ShadowBuilder::builder().build().unwrap();
    std::env::set_var("TZ", tz);
}
