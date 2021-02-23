use std::path::Path;

use peroxide::SodaBuilder;

macro_rules! f_p {
    ($r:expr) => {
        concat!("tests/audio/", $r)
    };
}

#[test]
fn weather() {
    test_audio(
        f_p!("whatstheweatherlike.wav"),
        "en_models",
        "what's the weather like",
    );
}

#[test]
fn blizzy() {
    test_audio(
        f_p!("quickfoxblizzy.wav"),
        "en_models",
        "the quick brown fox jumped over the lazy dog and Blizzy is short",
    );
}

fn test_audio<P: AsRef<Path>>(path: P, model: &str, expected: &str) {
    let mut data = std::fs::File::open(path).unwrap();

    let mut client = SodaBuilder::new()
        .channel_count(1)
        .sample_rate(16000)
        .language_pack_directory(String::from(model))
        .api_key("dummy-key".into())
        .build(|c: &str, f: bool| {
            if f {
                assert_eq!(c, expected);
            }
        });

    client.add_audio(&mut data);
}
