fn main() {
    println!("cargo:rerun-if-changed=assets/icons/app.ico");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/icons/app.ico")
            .set("FileDescription", "JetSeat Massage")
            .set("ProductName", "JetSeat Massage")
            .set(
                "LegalCopyright",
                "Copyright 2026 JetSeat Massage contributors",
            )
            .compile()
            .expect("embed Windows application icon and version information");
    }
}
