/*!
# How to use cpal

In order to play a sound, first you need to create a `Voice`.

```no_run
// getting the default sound output of the system (can return `None` if nothing is supported)
let endpoint = cpal::get_default_endpoint().unwrap();

// note that the user can at any moment disconnect the device, therefore all operations return
// a `Result` to handle this situation

// getting a format for the PCM
let format = endpoint.get_supported_formats_list().unwrap().next().unwrap();

let mut voice = cpal::Voice::new(&endpoint, &format).unwrap();
```

Then you must send raw samples to it by calling `append_data`. You must take the number of channels
and samples rate into account when writing the data.

TODO: add example

**Important**: the `append_data` function can return a buffer shorter than what you requested.
This is the case if the device doesn't have enough space available. **It happens very often**,
this is not some obscure situation that can be ignored.

After you have submitted data for the first time, call `play`:

```no_run
# let mut voice: cpal::Voice = unsafe { std::mem::uninitialized() };
voice.play();
```

The audio device of the user will read the buffer that you sent, and play it. If the audio device
reaches the end of the data, it will stop playing. You must continuously fill the buffer by
calling `append_data` repeatedly if you don't want the audio to stop playing.

*/

extern crate futures;
#[macro_use]
extern crate lazy_static;
extern crate libc;

pub use samples_formats::{SampleFormat, Sample};

#[cfg(all(not(windows), not(target_os = "linux"), not(target_os = "macos")))]
use null as cpal_impl;

use std::fmt;
use std::error::Error;
use std::ops::{Deref, DerefMut};

use futures::stream::Stream;
use futures::Poll;
use futures::Task;

mod null;
mod samples_formats;

#[cfg(target_os = "linux")]
#[path="alsa/mod.rs"]
mod cpal_impl;

#[cfg(windows)]
#[path="wasapi/mod.rs"]
mod cpal_impl;

#[cfg(target_os = "macos")]
#[path="coreaudio/mod.rs"]
mod cpal_impl;

/// An iterator for the list of formats that are supported by the backend.
pub struct EndpointsIterator(cpal_impl::EndpointsIterator);

impl Iterator for EndpointsIterator {
    type Item = Endpoint;

    #[inline]
    fn next(&mut self) -> Option<Endpoint> {
        self.0.next().map(Endpoint)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

/// Return an iterator to the list of formats that are supported by the system.
#[inline]
pub fn get_endpoints_list() -> EndpointsIterator {
    EndpointsIterator(Default::default())
}

/// Return the default endpoint, or `None` if no device is available.
#[inline]
pub fn get_default_endpoint() -> Option<Endpoint> {
    cpal_impl::get_default_endpoint().map(Endpoint)
}

/// An opaque type that identifies an end point.
#[derive(Clone, PartialEq, Eq)]
pub struct Endpoint(cpal_impl::Endpoint);

impl Endpoint {
    /// Returns an iterator that produces the list of formats that are supported by the backend.
    #[inline]
    pub fn get_supported_formats_list(&self) -> Result<SupportedFormatsIterator,
                                                       FormatsEnumerationError>
    {
        Ok(SupportedFormatsIterator(try!(self.0.get_supported_formats_list())))
    }

    /// Returns the name of the endpoint.
    #[inline]
    pub fn get_name(&self) -> String {
        self.0.get_name()
    }
}

/// Number of channels.
pub type ChannelsCount = u16;

/// Possible position of a channel.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ChannelPosition {
    FrontLeft,
    FrontRight,
    FrontCenter,
    LowFrequency,
    BackLeft,
    BackRight,
    FrontLeftOfCenter,
    FrontRightOfCenter,
    BackCenter,
    SideLeft,
    SideRight,
    TopCenter,
    TopFrontLeft,
    TopFrontCenter,
    TopFrontRight,
    TopBackLeft,
    TopBackCenter,
    TopBackRight,
}

///
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SamplesRate(pub u32);

/// Describes a format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Format {
    pub channels: Vec<ChannelPosition>,
    pub samples_rate: SamplesRate,
    pub data_type: SampleFormat,
}

/// An iterator that produces a list of formats supported by the endpoint.
pub struct SupportedFormatsIterator(cpal_impl::SupportedFormatsIterator);

impl Iterator for SupportedFormatsIterator {
    type Item = Format;

    #[inline]
    fn next(&mut self) -> Option<Format> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

pub struct EventLoop(cpal_impl::EventLoop);

impl EventLoop {
    #[inline]
    pub fn new() -> EventLoop {
        EventLoop(cpal_impl::EventLoop::new())
    }

    #[inline]
    pub fn run(&self) {
        self.0.run()
    }
}

/// Represents a buffer that must be filled with audio data.
///
/// You should destroy this object as soon as possible. Data is only committed when it
/// is destroyed.
#[must_use]
pub struct Buffer<T> where T: Sample {
    // also contains something, taken by `Drop`
    target: Option<cpal_impl::Buffer<T>>,
}

/// This is the struct that is provided to you by cpal when you want to write samples to a buffer.
///
/// Since the type of data is only known at runtime, you have to fill the right buffer.
pub enum UnknownTypeBuffer {
    /// Samples whose format is `u16`.
    U16(Buffer<u16>),
    /// Samples whose format is `i16`.
    I16(Buffer<i16>),
    /// Samples whose format is `f32`.
    F32(Buffer<f32>),
}

impl UnknownTypeBuffer {
    /// Returns the length of the buffer in number of samples.
    #[inline]
    pub fn len(&self) -> usize {
        match self {
            &UnknownTypeBuffer::U16(ref buf) => buf.target.as_ref().unwrap().len(),
            &UnknownTypeBuffer::I16(ref buf) => buf.target.as_ref().unwrap().len(),
            &UnknownTypeBuffer::F32(ref buf) => buf.target.as_ref().unwrap().len(),
        }
    }
}

/// Error that can happen when enumerating the list of supported formats.
#[derive(Debug)]
pub enum FormatsEnumerationError {
    /// The device no longer exists. This can happen if the device is disconnected while the
    /// program is running.
    DeviceNotAvailable,
}

impl fmt::Display for FormatsEnumerationError {
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(fmt, "{}", self.description())
    }
}

impl Error for FormatsEnumerationError {
    #[inline]
    fn description(&self) -> &str {
        match self {
            &FormatsEnumerationError::DeviceNotAvailable => {
                "The requested device is no longer available (for example, it has been unplugged)."
            },
        }
    }
}

/// Error that can happen when creating a `Voice`.
#[derive(Debug)]
pub enum CreationError {
    /// The device no longer exists. This can happen if the device is disconnected while the
    /// program is running.
    DeviceNotAvailable,

    /// The required format is not supported.
    FormatNotSupported,
}

impl fmt::Display for CreationError {
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(fmt, "{}", self.description())
    }
}

impl Error for CreationError {
    #[inline]
    fn description(&self) -> &str {
        match self {
            &CreationError::DeviceNotAvailable => {
                "The requested device is no longer available (for example, it has been unplugged)."
            },

            &CreationError::FormatNotSupported => {
                "The requested samples format is not supported by the device."
            },
        }
    }
}

/// Controls a sound output. A typical application has one `Voice` for each sound
/// it wants to output.
///
/// A voice must be periodically filled with new data by calling `append_data`, or the sound
/// will stop playing.
///
/// Each `Voice` is bound to a specific number of channels, samples rate, and samples format,
/// which can be retreived by calling `get_channels`, `get_samples_rate` and `get_samples_format`.
/// If you call `append_data` with values different than these, then cpal will automatically
/// perform a conversion on your data.
///
/// If you have the possibility, you should try to match the format of the voice.
pub struct Voice {
    voice: cpal_impl::Voice,
    format: Format,
}

impl Voice {
    /// Builds a new channel.
    #[inline]
    pub fn new(endpoint: &Endpoint, format: &Format, event_loop: &EventLoop)
               -> Result<(Voice, SamplesStream), CreationError>
    {
        let (voice, stream) = try!(cpal_impl::Voice::new(&endpoint.0, format, &event_loop.0));

        let voice = Voice {
            voice: voice,
            format: format.clone(),
        };

        let stream = SamplesStream(stream);

        Ok((voice, stream))
    }

    /// Returns the format used by the voice.
    #[inline]
    pub fn format(&self) -> &Format {
        &self.format
    }

    /// DEPRECATED: use `format` instead. Returns the number of channels.
    ///
    /// You can add data with any number of channels, but matching the voice's native format
    /// will lead to better performances.
    #[inline]
    pub fn get_channels(&self) -> ChannelsCount {
        self.format().channels.len() as ChannelsCount
    }

    /// DEPRECATED: use `format` instead. Returns the number of samples that are played per second.
    ///
    /// You can add data with any samples rate, but matching the voice's native format
    /// will lead to better performances.
    #[inline]
    pub fn get_samples_rate(&self) -> SamplesRate {
        self.format().samples_rate
    }

    /// DEPRECATED: use `format` instead. Returns the format of the samples that are accepted by the backend.
    ///
    /// You can add data of any format, but matching the voice's native format
    /// will lead to better performances.
    #[inline]
    pub fn get_samples_format(&self) -> SampleFormat {
        self.format().data_type
    }

    /// Sends a command to the audio device that it should start playing.
    ///
    /// Has no effect is the voice was already playing.
    ///
    /// Only call this after you have submitted some data, otherwise you may hear
    /// some glitches.
    #[inline]
    pub fn play(&mut self) {
        self.voice.play()
    }

    /// Sends a command to the audio device that it should stop playing.
    ///
    /// Has no effect is the voice was already paused.
    ///
    /// If you call `play` afterwards, the playback will resume exactly where it was.
    #[inline]
    pub fn pause(&mut self) {
        self.voice.pause()
    }
}

pub struct SamplesStream(cpal_impl::SamplesStream);

impl Stream for SamplesStream {
    type Item = UnknownTypeBuffer;
    type Error = ();

    #[inline]
    fn poll(&mut self, task: &mut Task) -> Poll<Option<Self::Item>, Self::Error> {
        self.0.poll(task)
    }

    #[inline]
    fn schedule(&mut self, task: &mut Task) {
        self.0.schedule(task)
    }
}

impl<T> Deref for Buffer<T> where T: Sample {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &[T] {
        panic!("It is forbidden to read from the audio buffer");
    }
}

impl<T> DerefMut for Buffer<T> where T: Sample {
    #[inline]
    fn deref_mut(&mut self) -> &mut [T] {
        self.target.as_mut().unwrap().get_buffer()
    }
}

impl<T> Drop for Buffer<T> where T: Sample {
    #[inline]
    fn drop(&mut self) {
        self.target.take().unwrap().finish();
    }
}
