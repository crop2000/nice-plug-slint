fn main() {
    println!("cargo:rerun-if-changed=src/gui/ui.slint");
    println!("cargo:rerun-if-changed=src/gui/DSL/knob.slint");
    slint_build::compile("src/gui/ui.slint").unwrap();
}
