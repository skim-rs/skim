use cfg_aliases::cfg_aliases;

fn main() {
    // Setup cfg aliases
    cfg_aliases! {
        // Platforms
        frizbee_simd: { any(target_arch = "x86_64", target_arch = "aarch64") },
        // Backends
        frizbee: { all(feature = "frizbee", frizbee_simd) }
    }
}
