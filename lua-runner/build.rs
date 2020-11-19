fn main() {
    let mut flags = vergen::ConstantsFlags::empty();
    flags.toggle(vergen::ConstantsFlags::BUILD_TIMESTAMP);
    vergen::generate_cargo_keys(flags).expect("Unable to generate the cargo keys!");
}
