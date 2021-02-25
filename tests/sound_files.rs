use std::path::Path;

use peroxide::{RecognitionMode, ResultType, SodaBuilder, SodaResponse};

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
        .recognition_mode(RecognitionMode::Caption)
        .language_pack_directory(String::from(model))
        .api_key("262603e4-b3c8-4398-b306-c1260751f8d9".into())
        .build(|r: SodaResponse| {
            if let Some(recognition_result) = r.recognition_result {
                if let Some(rt) = recognition_result.result_type {
                    if rt == ResultType::Final as i32 {
                        assert_eq!(recognition_result.hypothesis[0], expected);
                    }
                }
            }
        });

    client.add_simulated_audio(&mut data);
}
