#[cfg(not(debug_assertions))]
fn main() {
    use std::env;

    println!("cargo:rerun-if-changed=build.rs");

    let mut res = winres::WindowsResource::new();
    res.set("InternalName", &env::var("CARGO_PKG_NAME").unwrap())
        .set("FileDescription", &env::var("CARGO_PKG_DESCRIPTION").unwrap())
        .set("CompanyName", &env::var("CARGO_PKG_AUTHORS").unwrap())
        .set("LegalCopyright", &env::var("CARGO_PKG_AUTHORS").unwrap());
    res.compile().unwrap();
}

#[cfg(debug_assertions)]
fn main() {}
