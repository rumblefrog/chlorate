fn main() {
    prost_build::compile_protos(&["proto/soda_api.proto"], &["proto"]).unwrap();
}
