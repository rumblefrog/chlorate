use std::ffi::CStr;
use std::io::Read;
use std::marker::PhantomData;
use std::ops::Drop;

use libc::{c_char, c_int, c_void};

use prost::Message;

mod soda_api {
    include!(concat!(env!("OUT_DIR"), "/speech.soda.api.rs"));
}

use soda_api::SerializedSodaConfigMsg;

pub use soda_api::{
    serialized_soda_config_msg::RecognitionMode,
    soda_recognition_result::{FinalResultEndpointReason, ResultType},
    soda_response::SodaMessageType,
    SodaAudioLevelInfo, SodaEndpointEvent, SodaLangIdEvent, SodaRecognitionResult, SodaResponse,
};

// typedef void (*SerializedSodaEventHandler)(const char*, int, void*);
type SerializedSodaEventHandler = extern "C" fn(*const c_char, c_int, *mut c_void);

/// Soda config.
/// https://github.com/chromium/chromium/blob/02aa060d95c07ac400e4dcdc382dcf3f2c0beb9d/chrome/services/speech/soda/soda_async_impl.h#L40:3
#[repr(C)]
struct SerializedSodaConfig {
    /// A ExtendedSodaConfigMsg that's been serialized as a string. Not owned.
    soda_config: *const c_char,

    /// length of char* in soda_config.
    soda_config_size: c_int,

    /// The callback that gets executed on a SODA event. It takes in a
    /// char*, which is a serialized SodaResponse proto, an int specifying the
    /// length of the char* and a void pointer to the object that is associated
    /// with the callback.
    callback: SerializedSodaEventHandler,

    /// A void pointer to the object that is associated with the callback.
    callback_handle: *mut c_void,
}

#[link(name = "soda")]
extern "C" {
    fn CreateExtendedSodaAsync(config: SerializedSodaConfig) -> *mut c_void;
    fn DeleteExtendedSodaAsync(soda_async_handle: *mut c_void);
    fn ExtendedAddAudio(
        soda_async_handle: *mut c_void,
        audio_buffer: *const c_char,
        audio_buffer_size: c_int,
    );
    fn ExtendedSodaStart(soda_async_handle: *mut c_void);
}

type SodaCBFn<'soda> = dyn Fn(SodaResponse) + Send + Sync + 'soda;
type SodaCallback<'soda> = Box<Box<SodaCBFn<'soda>>>;

pub struct SodaBuilder {
    channel_count: u32,

    sample_rate: u32,

    max_buffer_bytes: u32,

    simulate_realtime_test_only: bool,

    language_pack_directory: String,

    api_key: String,

    recognition_mode: RecognitionMode,

    reset_on_final_result: bool,

    include_timing_metrics: bool,

    enable_lang_id: bool,
}

impl Default for SodaBuilder {
    fn default() -> SodaBuilder {
        SodaBuilder {
            channel_count: 1,
            sample_rate: 16000,
            language_pack_directory: "./SODAModels".into(),
            max_buffer_bytes: 0,
            simulate_realtime_test_only: false,
            api_key: "dummy_key".into(),
            recognition_mode: RecognitionMode::Ime,
            reset_on_final_result: true,
            include_timing_metrics: true,
            enable_lang_id: false,
        }
    }
}

impl SodaBuilder {
    pub fn new() -> SodaBuilder {
        SodaBuilder::default()
    }

    /// Number of channels in RAW audio that will be provided to SODA.
    pub fn channel_count(&mut self, channel_count: u32) -> &mut SodaBuilder {
        self.channel_count = channel_count;
        self
    }

    /// Maximum size of buffer to use in PipeStream. By default, is 0, which means
    /// unlimited.
    pub fn sample_rate(&mut self, sample_rate: u32) -> &mut SodaBuilder {
        self.sample_rate = sample_rate;
        self
    }

    /// Maximum size of buffer to use in PipeStream. By default, is 0, which means
    /// unlimited.
    pub fn max_buffer_bytes(&mut self, max_buffer_bytes: u32) -> &mut SodaBuilder {
        self.max_buffer_bytes = max_buffer_bytes;
        self
    }

    /// If set to true, forces the audio provider to simulate realtime audio
    /// provision. This only makes sense during testing, to simulate realtime audio
    /// providing from a big chunk of audio.
    /// This slows down audio provided to SODA to a maximum of real-time, which
    /// means more accurate endpointer behavior, but is unsuitable for execution in
    /// real production environments. Set with caution!
    pub fn simulate_realtime_testonly(
        &mut self,
        simulate_realtime_testonly: bool,
    ) -> &mut SodaBuilder {
        self.simulate_realtime_test_only = simulate_realtime_testonly;
        self
    }

    /// Directory of the language pack to use.
    pub fn language_pack_directory(&mut self, language_pack_directory: String) -> &mut SodaBuilder {
        self.language_pack_directory = language_pack_directory;
        self
    }

    /// API key used for call verification.
    pub fn api_key(&mut self, api_key: String) -> &mut SodaBuilder {
        self.api_key = api_key;
        self
    }

    /// What kind of recognition to execute here. Impacts model usage.
    pub fn recognition_mode(&mut self, recognition_mode: RecognitionMode) -> &mut SodaBuilder {
        self.recognition_mode = recognition_mode;
        self
    }

    /// Whether terse_processor should force a new session after every final
    /// recognition result.
    /// This will cause the terse processor to stop processing new audio once an
    /// endpoint event is detected and wait for it to generate a final event using
    /// audio up to the endpoint. This will cause processing bursts when a new
    /// session starts.
    pub fn reset_on_final_result(&mut self, reset_on_final_result: bool) -> &mut SodaBuilder {
        self.reset_on_final_result = reset_on_final_result;
        self
    }

    /// Whether to populate the timing_metrics field on Recognition and Endpoint
    /// events.
    pub fn include_timing_metrics(&mut self, include_timing_metrics: bool) -> &mut SodaBuilder {
        self.include_timing_metrics = include_timing_metrics;
        self
    }

    /// Whether or not to request lang id events.
    pub fn enable_lang_id(&mut self, enable_lang_id: bool) -> &mut SodaBuilder {
        self.enable_lang_id = enable_lang_id;
        self
    }

    /// Consumes `SodaBuilder` to create `SodaClient`.
    pub fn build<'soda>(
        &mut self,
        callback: impl Fn(SodaResponse) + Send + Sync + 'soda,
    ) -> SodaClient<'soda> {
        let callback: SodaCallback = Box::new(Box::new(callback));

        let config = SerializedSodaConfigMsg {
            channel_count: Some(self.channel_count as i32),
            sample_rate: Some(self.sample_rate as i32),
            max_buffer_bytes: Some(self.max_buffer_bytes as i32),
            simulate_realtime_testonly: Some(self.simulate_realtime_test_only),
            api_key: Some(self.api_key.clone()),
            language_pack_directory: Some(self.language_pack_directory.clone()),
            recognition_mode: Some(self.recognition_mode as i32),
            reset_on_final_result: Some(self.reset_on_final_result),
            include_timing_metrics: Some(self.include_timing_metrics),
            enable_lang_id: Some(self.enable_lang_id),
            ..Default::default()
        };

        let mut buf = Vec::new();

        config.encode(&mut buf).unwrap();

        let serialized = SerializedSodaConfig {
            soda_config: buf.as_ptr() as *const c_char,
            soda_config_size: buf.len() as i32,
            callback: soda_callback,
            callback_handle: Box::into_raw(callback) as *mut c_void,
        };

        let p = unsafe {
            let handle = CreateExtendedSodaAsync(serialized);

            ExtendedSodaStart(handle);

            handle
        };

        SodaClient {
            soda_handle: p,
            phantom: PhantomData,
        }
    }
}

pub struct SodaClient<'soda> {
    soda_handle: *mut c_void,

    phantom: PhantomData<&'soda ()>,
}

impl<'soda> SodaClient<'soda> {
    /// Adds audio to SODA processor in 2048 byte chunks.
    pub fn add_audio<R>(&mut self, data: R)
    where
        R: Read,
    {
        self.add_chunked_audio(data, false);
    }

    /// Adds audio to SODA processor in 2048 byte chunks.
    /// Includes 20ms delay for real-time audio simulation.
    pub fn add_simulated_audio<R>(&mut self, data: R)
    where
        R: Read,
    {
        self.add_chunked_audio(data, true);
    }

    fn add_chunked_audio<R>(&mut self, data: R, simulate_real_time: bool)
    where
        R: Read,
    {
        let mut data = data;

        let mut chunk = vec![0; 2048];

        while let Ok(len) = data.read(&mut chunk) {
            if len == 0 {
                break;
            }

            unsafe {
                ExtendedAddAudio(
                    self.soda_handle,
                    (&chunk[..len]).as_ptr() as *const c_char,
                    len as c_int,
                )
            };

            // Sleep for 20ms to simulate real-time audio. SODA requires audio
            // streaming in order to return events.
            if simulate_real_time {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
    }
}

impl<'soda> Drop for SodaClient<'soda> {
    fn drop(&mut self) {
        unsafe { DeleteExtendedSodaAsync(self.soda_handle) };
    }
}

extern "C" fn soda_callback(message: *const c_char, _length: c_int, callback: *mut c_void) {
    let buf = unsafe { CStr::from_ptr(message) };

    if let Ok(sr) = SodaResponse::decode(buf.to_bytes()) {
        let user_callback: *mut Box<SodaCBFn> = callback as *mut _;

        (unsafe { &*user_callback })(sr);
    }
}
