use std::ffi::CStr;
use std::io::Read;
use std::marker::PhantomData;
use std::ops::Drop;

use libc::{c_char, c_int, c_void};

// typedef void (*RecognitionResultHandler)(const char*, const bool, void*);
type RecognitionResultHandler = extern "C" fn(*const c_char, bool, *mut c_void);

/// Soda config.
/// https://github.com/chromium/chromium/blob/02aa060d95c07ac400e4dcdc382dcf3f2c0beb9d/chrome/services/speech/soda/soda_async_impl.h#L40:3
#[repr(C)]
struct SodaConfig {
    /// The channel count and sample rate of the audio stream. SODA does not
    /// support changing these values mid-stream, so a new SODA instance must be
    /// created if these values change.
    channel_count: c_int,
    sample_rate: c_int,

    /// The fully-qualified path to the language pack.
    language_pack_directory: *const c_char,

    /// The callback that gets executed on a recognition event. It takes in a
    /// char*, representing the transcribed text; a bool, representing whether the
    /// result is final or not; and a void pointer to the object that is associated
    /// with the callback.
    callback: RecognitionResultHandler,

    /// A void pointer to the object that is associated with the callback.
    /// Ownership is not taken.
    callback_handle: *mut c_void,

    /// The API key used to verify that the binary is called by Chrome.
    api_key: *const c_char,
}

#[link(name = "soda")]
extern "C" {
    fn CreateSodaAsync(config: SodaConfig) -> *mut c_void;
    fn DeleteSodaAsync(soda_async_handle: *mut c_void);
    fn AddAudio(
        soda_async_handle: *mut c_void,
        audio_buffer: *const c_char,
        audio_buffer_size: c_int,
    );
}

type SodaCBFn<'soda> = dyn Fn(&str, bool) + 'soda;
pub type SodaCallback<'soda> = Box<Box<SodaCBFn<'soda>>>;

pub struct SodaBuilder {
    channel_count: i32,

    sample_rate: i32,

    language_pack_directory: String,

    api_key: String,
}

impl SodaBuilder {
    pub fn new() -> SodaBuilder {
        SodaBuilder {
            channel_count: 1,
            sample_rate: 16000,
            language_pack_directory: "./SODAModels".into(),
            api_key: "dummy_key".into(),
        }
    }

    pub fn channel_count<'b>(&'b mut self, channel_count: u32) -> &'b mut SodaBuilder {
        self.channel_count = channel_count as i32;
        self
    }

    pub fn sample_rate<'b>(&'b mut self, sample_rate: u32) -> &'b mut SodaBuilder {
        self.sample_rate = sample_rate as i32;
        self
    }

    pub fn language_pack_directory<'b>(
        &'b mut self,
        language_pack_directory: String,
    ) -> &'b mut SodaBuilder {
        self.language_pack_directory = language_pack_directory;
        self
    }

    pub fn api_key<'b>(&'b mut self, api_key: String) -> &'b mut SodaBuilder {
        self.api_key = api_key;
        self
    }

    pub fn build<'soda>(&mut self, callback: impl Fn(&str, bool) + 'soda) -> SodaClient<'soda> {
        let callback: SodaCallback = Box::new(Box::new(callback));

        let c = SodaConfig {
            channel_count: self.channel_count,
            sample_rate: self.sample_rate,
            language_pack_directory: self.language_pack_directory.as_ptr() as *const c_char,
            callback: soda_callback,
            callback_handle: Box::into_raw(callback) as *mut c_void,
            api_key: self.api_key.as_ptr() as *const c_char,
        };

        let p = unsafe { CreateSodaAsync(c) };

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
    pub fn add_audio<R>(&mut self, data: R)
    where
        R: Read,
    {
        let mut data = data;

        let mut chunk = vec![0; 2048];

        while let Ok(len) = data.read(&mut chunk) {
            if len == 0 {
                break;
            }

            self.add_chunked_audio(&chunk[..len], len as u32);

            // Sleep for 20ms to simulate real-time audio. SODA requires audio
            // streaming in order to return events.
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    fn add_chunked_audio(&mut self, data: &[u8], len: u32) {
        unsafe {
            AddAudio(
                self.soda_handle,
                data.as_ptr() as *const c_char,
                len as c_int,
            )
        };
    }
}

impl<'soda> Drop for SodaClient<'soda> {
    fn drop(&mut self) {
        unsafe { DeleteSodaAsync(self.soda_handle) };
    }
}

extern "C" fn soda_callback(content: *const c_char, is_final: bool, callback: *mut c_void) {
    let user_callback: *mut Box<SodaCBFn> = callback as *mut _;

    let c = unsafe { CStr::from_ptr(content) };

    (unsafe { &*user_callback })(&c.to_string_lossy(), is_final);
}
