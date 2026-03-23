fn main() {
    for key in ["ONETCLI_UPDATE_URL", "ONETCLI_UPDATE_DOWNLOAD_URL"] {
        println!("cargo:rerun-if-env-changed={key}");
        if let Ok(val) = std::env::var(key) {
            if !val.is_empty() {
                println!("cargo:rustc-env={key}={val}");
            }
        }
    }
}
