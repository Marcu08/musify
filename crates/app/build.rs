#[cfg(windows)]
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=../../assets/musify.ico");

    #[cfg(windows)]
    {
        let icon = Path::new("../../assets/musify.ico");
        if !icon.is_file() {
            panic!("icon not found: {}", icon.display());
        }
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("../../assets/musify.ico");
        resource.set("ProductName", "Musify");
        resource.set("FileDescription", "Musify - il tuo player musicale desktop");
        resource.set("CompanyName", "Musify");
        resource.set("LegalCopyright", "MIT");
        resource.set("OriginalFilename", "musify.exe");
        resource.set("InternalName", "musify");
        resource.set("FileVersion", env!("CARGO_PKG_VERSION"));
        resource.set("ProductVersion", env!("CARGO_PKG_VERSION"));
        if let Err(error) = resource.compile() {
            panic!("cannot compile the Windows resources: {error}");
        }
    }
}
